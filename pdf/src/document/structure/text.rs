//! Explicit declared-order extraction. No geometric reading-order inference.
use super::{StructureAssociation, StructureContentDomain, StructureKid, StructureTree, invalid};
use crate::{
    document::{
        PdfDocument,
        identity::{DocumentStamp, ObjectHandle},
    },
    error::{PdfError, PdfResult},
    objects::object_id::ObjectId,
    page::{
        execution_budget::ExecutionBudget,
        marked_text::{
            ActualTextSource, MarkedTextOptions, MarkedTextSpan, TextContentLocation, add,
            joined_text,
        },
        source_provenance::FormInvocation,
    },
    parser::options::Limits,
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredTextSpan {
    pub element: ObjectHandle,
    pub span: MarkedTextSpan,
}
#[derive(Debug, Clone)]
pub struct StructuredTextReport {
    pub optional_content_selection:
        Option<crate::document::optional_content::OptionalContentSelection>,
    pub stamp: DocumentStamp,
    pub root: ObjectHandle,
    pub spans: Vec<StructuredTextSpan>,
    /// Retained paint-order units outside the declared structure, not inferred.
    pub unassociated_spans: usize,
    /// OBJR without an enclosing element ActualText. No AP/Alt text synthesis.
    pub unextracted_object_references: usize,
}
impl StructuredTextReport {
    pub fn text(&self, limits: &Limits) -> PdfResult<String> {
        joined_text(self.spans.iter().map(|s| &s.span), limits)
    }
}
type Key = (ObjectId, ObjectId, u32); // page, domain, MCID
#[derive(Clone, Copy)]
enum Child {
    Element(ObjectId),
    Content(usize),
}
fn domain(location: &TextContentLocation) -> PdfResult<ObjectId> {
    if let Some(call) = location.invocations.last() {
        call.form_id.ok_or_else(|| {
            PdfError::UnsupportedFeature("structured text requires indirect Form domains".into())
        })
    } else {
        Ok(location.page.object_id())
    }
}
fn key(association: &StructureAssociation) -> Option<Key> {
    match association {
        StructureAssociation::MarkedContent {
            page, domain, mcid, ..
        } => {
            let (StructureContentDomain::Page(handle) | StructureContentDomain::Form(handle)) =
                domain;
            Some((page.object_id(), handle.object_id(), *mcid))
        }
        _ => None,
    }
}
fn owner(association: &StructureAssociation) -> ObjectId {
    match association {
        StructureAssociation::MarkedContent { element, .. }
        | StructureAssociation::ObjectReference { element, .. } => element.object_id(),
    }
}
fn children(
    tree: &StructureTree,
    associations: &[StructureAssociation],
) -> PdfResult<HashMap<ObjectId, Vec<Child>>> {
    let mut by_owner: HashMap<_, Vec<usize>> = HashMap::new();
    for (i, association) in associations.iter().enumerate() {
        by_owner.entry(owner(association)).or_default().push(i);
    }
    let mut output = HashMap::new();
    for element in &tree.elements {
        let mut content = by_owner
            .get(&element.handle.object_id())
            .into_iter()
            .flatten();
        let mut kids = Vec::new();
        for kid in &element.children {
            kids.push(match kid {
                StructureKid::Element(h) => Child::Element(h.object_id()),
                StructureKid::UnvalidatedContent(_) => Child::Content(
                    *content
                        .next()
                        .ok_or_else(|| invalid("association traversal mismatch"))?,
                ),
            });
        }
        output.insert(element.handle.object_id(), kids);
    }
    Ok(output)
}
fn location_bytes(source: &TextContentLocation) -> usize {
    std::mem::size_of::<TextContentLocation>().saturating_add(
        source
            .invocations
            .len()
            .saturating_mul(std::mem::size_of::<FormInvocation>()),
    )
}
fn span_bytes(span: &MarkedTextSpan) -> usize {
    let mut size = span.text.as_ref().map_or(0, String::len);
    for source in &span.sources {
        size = size.saturating_add(location_bytes(source));
    }
    for content in &span.content_items {
        size = size.saturating_add(location_bytes(&content.location));
    }
    if let Some(ActualTextSource::MarkedContent(source)) = &span.replacement {
        size = size.saturating_add(location_bytes(source));
    }
    size
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Declared K order, separately from page paint order. None means no tree.
    /// Validates supported ownership before interpretation. Shared Form content
    /// with multiple physical instances and atomic replacements crossing owners
    /// reject rather than duplicating text or guessing an instance/order.
    pub fn extract_text_structure_order(
        &self,
        options: MarkedTextOptions,
    ) -> PdfResult<Option<StructuredTextReport>> {
        self.extract_text_structure_order_selected(options, None)
    }

    /// Declared order with explicit optional-content states. Structure-level
    /// ActualText is currently rejected rather than partitioned by layer.
    pub fn extract_text_structure_order_with_optional_content(
        &self,
        options: MarkedTextOptions,
        visibility: &crate::document::optional_content::OptionalContentVisibility,
    ) -> PdfResult<Option<StructuredTextReport>> {
        visibility.validate(self.ctx())?;
        self.extract_text_structure_order_selected(options, Some(visibility))
    }

    fn extract_text_structure_order_selected(
        &self,
        options: MarkedTextOptions,
        visibility: Option<&crate::document::optional_content::OptionalContentVisibility>,
    ) -> PdfResult<Option<StructuredTextReport>> {
        let Some(tree) = self.structure_tree()? else {
            return Ok(None);
        };
        if visibility.is_some()
            && options.use_actual_text
            && tree.elements.iter().any(|e| e.actual_text.is_some())
        {
            return Err(PdfError::UnsupportedFeature("structure ActualText with optional-content filtering requires explicit partitioning".into()));
        }
        let associations = self
            .structure_associations()?
            .ok_or_else(|| invalid("structure disappeared during extraction"))?;
        let limits = self.ctx().limits();
        let kids = children(&tree, &associations.items)?;
        let nodes: HashMap<_, _> = tree
            .elements
            .iter()
            .map(|e| (e.handle.object_id(), e))
            .collect();
        let mut owners = HashMap::new();
        for association in &associations.items {
            if let Some(key) = key(association) {
                owners.insert(key, owner(association));
            }
        }
        let work = ExecutionBudget::new();
        let mut spans: Vec<Option<MarkedTextSpan>> = Vec::new();
        let mut by_content: HashMap<Key, Vec<usize>> = HashMap::new();
        let mut instances: HashMap<(ObjectId, u32), (ObjectId, Vec<FormInvocation>)> =
            HashMap::new();
        let mut bytes = 0usize;
        let mut entries = 0usize;
        let mut chars = 0usize;
        let mut unassociated = 0usize;
        for page in self.collect_page_entries()? {
            let mut page = crate::page::PdfPage::new(
                crate::objects::PdfObject::PdfReference(
                    crate::objects::pdf_reference::PdfReference::from_id(page.page_id),
                ),
                self.ctx(),
            );
            let report = page.extract_marked_text_selected(options, work.clone(), visibility)?;
            if !report.diagnostics.is_empty() {
                return Err(PdfError::UnsupportedFeature(
                    "structured text requires interpretation without diagnostics".into(),
                ));
            }
            for span in report.spans {
                bytes = add(
                    bytes,
                    span_bytes(&span),
                    limits.max_decoded_bytes,
                    "structured retained bytes",
                )?;
                entries = add(
                    entries,
                    1usize
                        .saturating_add(span.sources.len())
                        .saturating_add(span.content_items.len()),
                    limits.max_objects,
                    "structured entries",
                )?;
                chars = add(
                    chars,
                    span.text.as_ref().map_or(0, |s| s.chars().count()),
                    limits.max_text_chars,
                    "structured captured characters",
                )?;
                let mut keys = HashSet::new();
                let mut element = None;
                for content in &span.content_items {
                    let domain = domain(&content.location)?;
                    let key = (content.location.page.object_id(), domain, content.mcid);
                    let id = owners.get(&key).copied().ok_or_else(|| {
                        PdfError::UnsupportedFeature(
                            "structured MCID has no unique declared page/Form association".into(),
                        )
                    })?;
                    if element.is_some_and(|old| old != id) {
                        return Err(PdfError::UnsupportedFeature(
                            "atomic ActualText crosses structure element owners".into(),
                        ));
                    }
                    element = Some(id);
                    if !content.location.invocations.is_empty() {
                        let instance = (
                            content.location.page.object_id(),
                            content.location.invocations.clone(),
                        );
                        if instances
                            .get(&(domain, content.mcid))
                            .is_some_and(|old| old != &instance)
                        {
                            return Err(PdfError::UnsupportedFeature(
                                "structured text has multiple shared Form instances".into(),
                            ));
                        }
                        instances.entry((domain, content.mcid)).or_insert(instance);
                    }
                    keys.insert(key);
                }
                if keys.is_empty() {
                    unassociated = add(unassociated, 1, limits.max_objects, "unassociated units")?;
                    continue;
                }
                let index = spans.len();
                for key in keys {
                    by_content.entry(key).or_default().push(index);
                }
                spans.push(Some(span));
            }
        }
        let mut tasks: Vec<_> = tree
            .children
            .iter()
            .rev()
            .filter_map(|kid| {
                if let StructureKid::Element(handle) = kid {
                    Some(Child::Element(handle.object_id()))
                } else {
                    None
                }
            })
            .collect();
        let mut output = Vec::new();
        let mut object_refs = 0usize;
        while let Some(child) = tasks.pop() {
            match child {
                Child::Element(id) => {
                    let element = nodes[&id];
                    if !options.include_artifacts
                        && element.resolved_role.namespace_uri == "http://iso.org/pdf2/ssn"
                        && element.resolved_role.name == "Artifact"
                    {
                        continue;
                    }
                    if options.use_actual_text && element.actual_text.is_some() {
                        let text = element.actual_text.clone();
                        chars = add(
                            chars,
                            text.as_ref().unwrap().chars().count(),
                            limits.max_text_chars,
                            "structured replacement characters",
                        )?;
                        let mut span = MarkedTextSpan {
                            text,
                            replacement: Some(ActualTextSource::StructureElement(
                                element.handle.clone(),
                            )),
                            sources: Vec::new(),
                            content_items: Vec::new(),
                        };
                        let mut pending = kids[&id].clone();
                        pending.reverse();
                        let mut seen_sources = HashSet::new();
                        let mut seen_content = HashSet::new();
                        while let Some(kid) = pending.pop() {
                            match kid {
                                Child::Element(id) => {
                                    pending.extend(kids[&id].iter().rev().copied())
                                }
                                Child::Content(index) => {
                                    if let Some(key) = key(&associations.items[index]) {
                                        for index in by_content.get(&key).into_iter().flatten() {
                                            if let Some(raw) = spans[*index].take() {
                                                for source in raw.sources {
                                                    if seen_sources.insert(source.clone()) {
                                                        span.sources.push(source);
                                                    }
                                                }
                                                for content in raw.content_items {
                                                    if seen_content.insert(content.clone()) {
                                                        span.content_items.push(content);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        bytes = add(
                            bytes,
                            span_bytes(&span),
                            limits.max_decoded_bytes,
                            "structured replacement bytes",
                        )?;
                        entries = add(
                            entries,
                            span.sources
                                .len()
                                .saturating_add(span.content_items.len())
                                .saturating_add(1),
                            limits.max_objects,
                            "structured replacement entries",
                        )?;
                        output.push(StructuredTextSpan {
                            element: element.handle.clone(),
                            span,
                        });
                    } else {
                        tasks.extend(kids[&id].iter().rev().copied());
                    }
                }
                Child::Content(index) => {
                    let association = &associations.items[index];
                    if let Some(key) = key(association) {
                        for index in by_content.get(&key).into_iter().flatten() {
                            if let Some(span) = spans[*index].take() {
                                output.push(StructuredTextSpan {
                                    element: nodes[&owner(association)].handle.clone(),
                                    span,
                                });
                            }
                        }
                    } else {
                        object_refs = add(
                            object_refs,
                            1,
                            limits.max_objects,
                            "unextracted object references",
                        )?;
                    }
                }
            }
        }
        Ok(Some(StructuredTextReport {
            optional_content_selection: visibility.map(|v| v.selection().clone()),
            stamp: tree.stamp,
            root: tree.root,
            spans: output,
            unassociated_spans: unassociated,
            unextracted_object_references: object_refs,
        }))
    }
}
