//! Opt-in logical text units. No synthetic glyph coordinates or edit addresses.
use super::{
    PdfPage, content_interpreter::Interpreter, content_parser::ContentParser,
    source_provenance::FormInvocation, text_object::TextObject,
};
use crate::{
    document::identity::{DocumentStamp, ObjectHandle},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    parser::{options::Limits, syntax::SyntaxParser},
};
use std::{
    cell::RefCell,
    io::{Cursor, Read, Seek},
    rc::Rc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkedTextOptions {
    pub use_actual_text: bool,
    pub include_artifacts: bool,
}
impl Default for MarkedTextOptions {
    fn default() -> Self {
        Self {
            use_actual_text: true,
            include_artifacts: false,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextContentLocation {
    pub page: ObjectHandle,
    pub invocations: Vec<FormInvocation>,
    pub operation_index: usize,
    pub operator_byte_offset: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarkedContentReference {
    pub mcid: u32,
    pub location: TextContentLocation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActualTextSource {
    MarkedContent(TextContentLocation),
    StructureElement(ObjectHandle),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkedTextSpan {
    /// None is an unmapped source glyph, never a guessed empty string.
    pub text: Option<String>,
    pub replacement: Option<ActualTextSource>,
    /// Original text-show operations covered by this logical unit.
    pub sources: Vec<TextContentLocation>,
    /// Innermost structural MCIDs covered by this indivisible unit.
    pub content_items: Vec<MarkedContentReference>,
}
#[derive(Debug, Clone)]
pub struct MarkedTextReport {
    pub optional_content_selection:
        Option<crate::document::optional_content::OptionalContentSelection>,
    pub stamp: DocumentStamp,
    pub page: ObjectHandle,
    pub spans: Vec<MarkedTextSpan>,
    pub diagnostics: Vec<super::diagnostic::ContentDiagnostic>,
}
impl MarkedTextReport {
    pub fn text(&self, limits: &Limits) -> PdfResult<String> {
        if !self.diagnostics.is_empty() {
            return Err(PdfError::UnsupportedFeature(
                "logical text has content diagnostics".into(),
            ));
        }
        joined_text(self.spans.iter(), limits)
    }
}
pub(crate) fn joined_text<'a>(
    spans: impl Iterator<Item = &'a MarkedTextSpan>,
    limits: &Limits,
) -> PdfResult<String> {
    let mut output = String::new();
    let mut chars = 0usize;
    for span in spans {
        let value = span.text.as_ref().ok_or_else(|| {
            PdfError::UnsupportedFeature("logical text contains an unmapped glyph".into())
        })?;
        chars = add(
            chars,
            value.chars().count(),
            limits.max_text_chars,
            "output Unicode characters",
        )?;
        add(
            output.len(),
            value.len(),
            limits.max_decoded_bytes,
            "output Unicode bytes",
        )?;
        output
            .try_reserve(value.len())
            .map_err(|_| PdfError::ResourceLimit("logical text allocation".into()))?;
        output.push_str(value);
    }
    Ok(output)
}
pub(crate) fn add(used: usize, amount: usize, limit: usize, what: &str) -> PdfResult<usize> {
    used.checked_add(amount)
        .filter(|v| *v <= limit)
        .ok_or_else(|| PdfError::ResourceLimit(format!("marked text {what}")))
}

#[derive(Debug)]
pub(crate) struct Mark {
    pub tag: String,
    pub visible: bool,
    pub mcid: Option<u32>,
    pub actual_text: Option<String>,
    pub location: TextContentLocation,
}
#[derive(Debug, Clone, Copy)]
enum Item {
    Mark(usize),
    Text(usize),
}
#[derive(Debug)]
struct Node {
    mark: Mark,
    children: Vec<Item>,
    parent: Option<usize>,
}
#[derive(Debug)]
pub(crate) struct Capture {
    limits: Limits,
    work: super::execution_budget::ExecutionBudget,
    nodes: Vec<Node>,
    texts: Vec<Option<MarkedTextSpan>>,
    text_visibility: Vec<bool>,
    roots: Vec<Item>,
    stack: Vec<usize>,
    bytes: usize,
    entries: usize,
    chars: usize,
}
impl Capture {
    fn new(limits: Limits, work: super::execution_budget::ExecutionBudget) -> Self {
        Self {
            limits,
            work,
            nodes: Vec::new(),
            texts: Vec::new(),
            text_visibility: Vec::new(),
            roots: Vec::new(),
            stack: Vec::new(),
            bytes: 0,
            entries: 0,
            chars: 0,
        }
    }
    pub fn metadata(&mut self, bytes: usize) -> PdfResult<()> {
        self.work.semantic_bytes(bytes, &self.limits)?;
        self.bytes = add(
            self.bytes,
            bytes,
            self.limits.max_decoded_bytes,
            "retained bytes",
        )?;
        Ok(())
    }
    fn entry(&mut self) -> PdfResult<()> {
        self.work.semantic_entries(1, &self.limits)?;
        self.entries = add(self.entries, 1, self.limits.max_objects, "entries")?;
        Ok(())
    }
    fn location(&mut self, location: &TextContentLocation) -> PdfResult<()> {
        self.metadata(
            std::mem::size_of::<TextContentLocation>().saturating_add(
                location
                    .invocations
                    .len()
                    .saturating_mul(std::mem::size_of::<FormInvocation>()),
            ),
        )
    }
    fn append(&mut self, item: Item) {
        if let Some(index) = self.stack.last() {
            self.nodes[*index].children.push(item);
        } else {
            self.roots.push(item);
        }
    }
    pub fn begin(&mut self, mark: Mark) -> PdfResult<()> {
        if self.stack.len() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit("marked text nesting".into()));
        }
        self.entry()?;
        self.metadata(mark.tag.len())?;
        self.location(&mark.location)?;
        if let Some(text) = &mark.actual_text {
            self.work
                .semantic_chars(text.chars().count(), &self.limits)?;
            self.chars = add(
                self.chars,
                text.chars().count(),
                self.limits.max_text_chars,
                "ActualText characters",
            )?;
        }
        let index = self.nodes.len();
        let parent = self.stack.last().copied();
        self.append(Item::Mark(index));
        self.nodes.push(Node {
            mark,
            children: Vec::new(),
            parent,
        });
        self.stack.push(index);
        Ok(())
    }
    pub fn end(&mut self) -> PdfResult<()> {
        self.stack
            .pop()
            .ok_or_else(|| PdfError::PageOperatorError("marked capture unmatched EMC".into()))?;
        Ok(())
    }
    fn content_item(&self, stack: &[usize]) -> Vec<MarkedContentReference> {
        stack
            .iter()
            .rev()
            .find_map(|index| {
                let mark = &self.nodes[*index].mark;
                mark.mcid.map(|mcid| {
                    vec![MarkedContentReference {
                        mcid,
                        location: mark.location.clone(),
                    }]
                })
            })
            .unwrap_or_default()
    }
    fn content_for_node(&self, mut index: usize) -> Vec<MarkedContentReference> {
        loop {
            let node = &self.nodes[index];
            if let Some(mcid) = node.mark.mcid {
                return vec![MarkedContentReference {
                    mcid,
                    location: node.mark.location.clone(),
                }];
            }
            if let Some(parent) = node.parent {
                index = parent;
            } else {
                return Vec::new();
            }
        }
    }
    pub fn text(
        &mut self,
        text: &TextObject,
        location: TextContentLocation,
        visible: bool,
    ) -> PdfResult<()> {
        for glyph in &text.items {
            self.entry()?;
            self.location(&location)?;
            let value = glyph.unicode().cloned();
            if let Some(value) = &value {
                self.work
                    .semantic_chars(value.chars().count(), &self.limits)?;
                self.metadata(value.len())?;
                self.chars = add(
                    self.chars,
                    value.chars().count(),
                    self.limits.max_text_chars,
                    "captured Unicode characters",
                )?;
            }
            let content_items = self.content_item(&self.stack);
            for item in &content_items {
                self.location(&item.location)?;
            }
            let index = self.texts.len();
            self.append(Item::Text(index));
            self.text_visibility.push(visible);
            self.texts.push(Some(MarkedTextSpan {
                text: value,
                replacement: None,
                sources: vec![location.clone()],
                content_items,
            }));
        }
        Ok(())
    }
    fn report(mut self, options: MarkedTextOptions) -> PdfResult<Vec<MarkedTextSpan>> {
        if !self.stack.is_empty() {
            return Err(PdfError::PageOperatorError(
                "marked capture unterminated sequence".into(),
            ));
        }
        let mut pending: Vec<_> = self.roots.iter().rev().map(|item| (*item, false)).collect();
        let mut output = Vec::new();
        while let Some((item, artifact)) = pending.pop() {
            match item {
                Item::Text(index) => {
                    if self.text_visibility[index] && (!artifact || options.include_artifacts) {
                        output.push(self.texts[index].take().unwrap());
                    }
                }
                Item::Mark(index) => {
                    if !self.nodes[index].mark.visible {
                        continue;
                    }
                    let artifact = artifact || self.nodes[index].mark.tag == "Artifact";
                    if artifact && !options.include_artifacts {
                        continue;
                    }
                    if options.use_actual_text && self.nodes[index].mark.actual_text.is_some() {
                        // An outer replacement is an atomic unit, including any
                        // nested replacement. Do not invent a character partition.
                        let mut span = MarkedTextSpan {
                            text: self.nodes[index].mark.actual_text.take(),
                            replacement: Some(ActualTextSource::MarkedContent(
                                self.nodes[index].mark.location.clone(),
                            )),
                            sources: Vec::new(),
                            content_items: self.content_for_node(index),
                        };
                        if let Some(ActualTextSource::MarkedContent(location)) = &span.replacement {
                            self.location(location)?;
                        }
                        for item in &span.content_items {
                            self.location(&item.location)?;
                        }
                        let mut descendants = vec![Item::Mark(index)];
                        let mut seen_content: std::collections::HashSet<_> =
                            span.content_items.iter().cloned().collect();
                        while let Some(item) = descendants.pop() {
                            match item {
                                Item::Text(i) => {
                                    if !self.text_visibility[i] {
                                        return Err(PdfError::UnsupportedFeature("ActualText spans hidden optional content; cannot partition an atomic replacement".into()));
                                    }
                                    let raw = self.texts[i].take().unwrap();
                                    for source in raw.sources {
                                        if span.sources.last() != Some(&source) {
                                            self.location(&source)?;
                                            span.sources.push(source);
                                        }
                                    }
                                    for content in raw.content_items {
                                        if seen_content.insert(content.clone()) {
                                            self.location(&content.location)?;
                                            span.content_items.push(content);
                                        }
                                    }
                                }
                                Item::Mark(i) => {
                                    if !self.nodes[i].mark.visible {
                                        return Err(PdfError::UnsupportedFeature("ActualText spans hidden optional content; cannot partition an atomic replacement".into()));
                                    }
                                    if self.nodes[i].children.is_empty() {
                                        for content in self.content_for_node(i) {
                                            if seen_content.insert(content.clone()) {
                                                self.location(&content.location)?;
                                                span.content_items.push(content);
                                            }
                                        }
                                    }
                                    descendants
                                        .extend(self.nodes[i].children.iter().rev().copied());
                                }
                            }
                        }
                        self.entry()?;
                        output.push(span);
                    } else {
                        pending.extend(
                            self.nodes[index]
                                .children
                                .iter()
                                .rev()
                                .map(|item| (*item, artifact)),
                        );
                    }
                }
            }
        }
        Ok(output)
    }
}

impl<R: Read + Seek> PdfPage<'_, R> {
    /// Opt-in logical units in content execution order. Default APIs are unchanged.
    pub fn extract_marked_text(
        &mut self,
        options: MarkedTextOptions,
    ) -> PdfResult<MarkedTextReport> {
        self.extract_marked_text_with_work(options, super::execution_budget::ExecutionBudget::new())
    }
    pub(crate) fn extract_marked_text_with_work(
        &mut self,
        options: MarkedTextOptions,
        work: super::execution_budget::ExecutionBudget,
    ) -> PdfResult<MarkedTextReport> {
        self.extract_marked_text_selected(options, work, None)
    }

    /// Explicit visibility filtering; hidden bytes are still executed and
    /// validated. Existing unfiltered extraction retains its original contract.
    pub fn extract_marked_text_with_optional_content(
        &mut self,
        options: MarkedTextOptions,
        visibility: &crate::document::optional_content::OptionalContentVisibility,
    ) -> PdfResult<MarkedTextReport> {
        self.extract_marked_text_selected(
            options,
            super::execution_budget::ExecutionBudget::new(),
            Some(visibility),
        )
    }

    pub(crate) fn extract_marked_text_selected(
        &mut self,
        options: MarkedTextOptions,
        work: super::execution_budget::ExecutionBudget,
        visibility: Option<&crate::document::optional_content::OptionalContentVisibility>,
    ) -> PdfResult<MarkedTextReport> {
        if let Some(visibility) = visibility {
            visibility.validate(self.ctx)?;
        }
        self.update_dimensions()?;
        let provenance = self.source_provenance();
        let page = ObjectHandle {
            document: self.ctx.identity(),
            id: provenance.page_id.ok_or_else(|| {
                PdfError::DocumentError("marked extraction requires indirect page".into())
            })?,
        };
        let bytes = self.content_bytes()?;
        work.decoded(bytes.len(), &self.ctx.limits())?;
        let parser = ContentParser::new(SyntaxParser::with_options(
            StreamReader::try_new(Cursor::new(bytes))?,
            self.ctx.options(),
        ));
        let capture = Rc::new(RefCell::new(Capture::new(self.ctx.limits(), work.clone())));
        let mut interpreter = Interpreter::new(self, self.ctx, parser).with_execution_budget(work);
        if let Some(visibility) = visibility {
            interpreter = interpreter.with_optional_content(visibility)?;
        }
        interpreter.capture_marked_text(capture.clone());
        // Normal interpretation validates fonts/graphics and bounds work; the
        // capture never changes the display objects or rendering state.
        let list = interpreter.run_to_display_list()?;
        drop(interpreter);
        let capture = Rc::try_unwrap(capture)
            .map_err(|_| PdfError::DocumentError("marked capture still borrowed".into()))?
            .into_inner();
        Ok(MarkedTextReport {
            optional_content_selection: visibility.map(|v| v.selection().clone()),
            stamp: provenance.stamp,
            page,
            spans: capture.report(options)?,
            diagnostics: list.diagnostics().to_vec(),
        })
    }
}
