//! Transactional editing of the supported declared structure graph.
use super::{StructureAssociation, StructureContentDomain, StructureKid, indirect, invalid};
use crate::{
    document::{
        PdfDocument,
        identity::{DocumentStamp, ObjectHandle},
        trees::{TreeKey, TreeKind},
    },
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
    page::content_program::ContentProgram,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Read, Seek},
    ops::Range,
};
mod transfer;

/// Explicit local attributes. Values must be direct, bounded data, never
/// references or streams. Namespace/opaque attributes can be preserved, not authored here.
#[derive(Debug, Clone)]
pub struct StructureAttributeWrite {
    pub owner: String,
    pub values: PdfDict,
}
#[derive(Debug, Clone)]
pub struct StructureElementDefinition {
    pub role: String,
    pub title: Option<String>,
    pub language: Option<String>,
    pub alternate_text: Option<String>,
    pub actual_text: Option<String>,
    pub attributes: Vec<StructureAttributeWrite>,
}
impl StructureElementDefinition {
    pub fn new(role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            title: None,
            language: None,
            alternate_text: None,
            actual_text: None,
            attributes: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureRemoval {
    /// Refuse any children, including content items.
    EmptyOnly,
    /// Remove only this node. Keep its children, order and content, changing
    /// their owner to the surviving parent. Root cannot own content directly.
    PromoteChildren,
}
/// A page-domain sequence allocated by mark_structure_content().
#[derive(Debug, Clone)]
pub struct StructureMark {
    pub stamp: DocumentStamp,
    pub element: ObjectHandle,
    pub page: ObjectHandle,
    pub mcid: u32,
}
#[derive(Clone)]
enum Kid {
    Element(ObjectId),
    Content {
        raw: PdfObject,
        association: StructureAssociation,
    },
}
struct Node {
    dict: PdfDict,
    parent: ObjectId,
    kids: Vec<Kid>,
}
struct Graph {
    root: ObjectId,
    dict: PdfDict,
    kids: Vec<Kid>,
    nodes: HashMap<ObjectId, Node>,
    reverse: BTreeMap<u32, PdfObject>,
    reverse_dict: PdfDict,
}
impl Graph {
    fn children(&mut self, parent: ObjectId) -> PdfResult<&mut Vec<Kid>> {
        if parent == self.root {
            Ok(&mut self.kids)
        } else {
            Ok(&mut self
                .nodes
                .get_mut(&parent)
                .ok_or_else(|| invalid("unknown structure parent"))?
                .kids)
        }
    }
    fn node(&self, id: ObjectId) -> PdfResult<&Node> {
        self.nodes
            .get(&id)
            .ok_or_else(|| invalid("handle is not a live structure element"))
    }
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn name(value: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(value.into()))
}
fn number(value: u32) -> PdfResult<PdfObject> {
    Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
        value.to_string().as_bytes(),
    )?))
}
fn array(values: impl IntoIterator<Item = PdfObject>) -> PdfResult<PdfObject> {
    let mut result = PdfArray::default();
    for value in values {
        result.try_add_obj(value)?;
    }
    Ok(PdfObject::PdfArray(result))
}
fn known(dict: &PdfDict, keys: &[&str]) -> PdfResult<()> {
    if dict
        .into_iter()
        .any(|(key, _)| !keys.contains(&key.as_str()))
    {
        return Err(PdfError::UnsupportedFeature(
            "structure editing with unknown/reverse-reference dictionary keys".into(),
        ));
    }
    Ok(())
}

impl<R: Read + Seek> PdfDocument<R> {
    fn require_structure_edit(&self) -> PdfResult<()> {
        self.ctx().require_content_edit_permission()?;
        if self.effective_version()? < crate::document::save_options::PdfVersion::V1_4 {
            return Err(PdfError::UnsupportedFeature("structure authoring requires PDF 1.4 or later; explicitly upgrade and reopen first".into()));
        }
        Ok(())
    }

    fn editable_structure(&self) -> PdfResult<Option<Graph>> {
        let Some(tree) = self.structure_tree()? else {
            return Ok(None);
        };
        let associations = self
            .structure_associations()?
            .ok_or_else(|| invalid("missing associations"))?;
        let dict = self.structure_dict(tree.root.object_id())?;
        known(
            &dict,
            &[
                "Type",
                "K",
                "RoleMap",
                "ClassMap",
                "Namespaces",
                "ParentTree",
                "ParentTreeNextKey",
            ],
        )?;
        let reverse_dict = dict
            .get("ParentTree")
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .into_dict()
                    .ok_or_else(|| invalid("ParentTree dictionary"))
            })
            .transpose()?
            .unwrap_or_default();
        // Rebuilding the number tree must not drop unknown keys at any level.
        let mut pending = vec![reverse_dict.clone()];
        let mut visits = 0usize;
        while let Some(node) = pending.pop() {
            visits += 1;
            if visits > self.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("structure tree nodes".into()));
            }
            known(&node, &["Nums", "Kids", "Limits"])?;
            if let Some(kids) = node.get("Kids") {
                let kids = self.ctx().resolve(kids)?;
                for kid in kids.as_array().ok_or_else(|| invalid("ParentTree Kids"))? {
                    pending.push(
                        self.ctx()
                            .resolve(kid)?
                            .into_dict()
                            .ok_or_else(|| invalid("ParentTree child"))?,
                    );
                }
            }
        }
        let reverse = tree
            .unvalidated_parent_tree
            .as_ref()
            .map(|v| self.read_tree(v, TreeKind::Number))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(|(k, v)| {
                let TreeKey::Number(k) = k else {
                    unreachable!()
                };
                (k, v)
            })
            .collect();
        let mut by_owner: HashMap<ObjectId, Vec<StructureAssociation>> = HashMap::new();
        for item in associations.items {
            let element = match &item {
                StructureAssociation::MarkedContent { element, .. }
                | StructureAssociation::ObjectReference { element, .. } => element.object_id(),
            };
            by_owner.entry(element).or_default().push(item);
        }
        let mut nodes = HashMap::new();
        for element in tree.elements {
            let id = element.handle.object_id();
            let dict = self.structure_dict(id)?;
            known(
                &dict,
                &[
                    "Type",
                    "S",
                    "P",
                    "K",
                    "Pg",
                    "T",
                    "Lang",
                    "Alt",
                    "ActualText",
                    "NS",
                    "R",
                    "A",
                    "C",
                ],
            )?;
            let mut items = by_owner.remove(&id).unwrap_or_default().into_iter();
            let mut kids = Vec::new();
            for kid in element.children {
                kids.push(match kid {
                    StructureKid::Element(h) => Kid::Element(h.object_id()),
                    StructureKid::UnvalidatedContent(raw) => {
                        let association = items
                            .next()
                            .ok_or_else(|| invalid("content traversal mismatch"))?;
                        // Make Pg explicit before moving content away from its old owner.
                        let mut raw = self.ctx().resolve(&raw)?.into_dict().unwrap_or_default();
                        let page = match &association {
                            StructureAssociation::MarkedContent { page, mcid, .. } => {
                                raw.try_insert("Type".into(), name("MCR"))?;
                                raw.try_insert("MCID".into(), number(*mcid)?)?;
                                page
                            }
                            StructureAssociation::ObjectReference { page, .. } => page,
                        };
                        known(&raw, &["Type", "Pg", "Stm", "MCID", "Obj"])?;
                        raw.try_insert("Pg".into(), reference(page.object_id()))?;
                        Kid::Content {
                            raw: PdfObject::PdfDict(raw),
                            association,
                        }
                    }
                });
            }
            nodes.insert(
                id,
                Node {
                    dict,
                    parent: element.parent.object_id(),
                    kids,
                },
            );
        }
        Ok(Some(Graph {
            root: tree.root.object_id(),
            dict,
            kids: tree
                .children
                .into_iter()
                .map(|k| {
                    let StructureKid::Element(h) = k else {
                        unreachable!()
                    };
                    Kid::Element(h.object_id())
                })
                .collect(),
            nodes,
            reverse,
            reverse_dict,
        }))
    }

    fn structure_definition(
        &self,
        dict: &mut PdfDict,
        value: &StructureElementDefinition,
    ) -> PdfResult<()> {
        let limits = self.ctx().limits();
        self.structure_edit_name(&value.role)?;
        dict.try_insert("S".into(), name(&value.role))?;
        for (key, text) in [
            ("T", &value.title),
            ("Lang", &value.language),
            ("Alt", &value.alternate_text),
            ("ActualText", &value.actual_text),
        ] {
            dict.remove(key);
            if let Some(text) = text {
                dict.try_insert(
                    key.into(),
                    PdfObject::PdfString(PdfString::from_text_with_limits(text, &limits)?),
                )?;
            }
        }
        let mut entries = 0usize;
        let mut bytes = 0usize;
        let mut attributes = Vec::new();
        let mut seen = HashSet::new();
        let revision = dict
            .get("R")
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_number()
                    .ok_or_else(|| invalid("element revision"))?
                    .as_u32_checked()
            })
            .transpose()?
            .unwrap_or(0);
        if value.attributes.len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("structure attribute count".into()));
        }
        for attribute in &value.attributes {
            self.structure_edit_name(&attribute.owner)?;
            bytes = bytes.saturating_add(attribute.owner.len());
            if bytes > limits.max_decoded_bytes {
                return Err(PdfError::ResourceLimit("structure attribute bytes".into()));
            }
            if attribute.owner == "NSO" {
                return Err(PdfError::UnsupportedFeature(
                    "namespace attribute authoring".into(),
                ));
            }
            let mut pending: Vec<_> = (&attribute.values)
                .into_iter()
                .map(|(k, v)| (k.as_str(), v, 0usize))
                .collect();
            while let Some((key, val, depth)) = pending.pop() {
                entries = entries
                    .checked_add(1)
                    .ok_or_else(|| invalid("attribute entries overflow"))?;
                bytes = bytes
                    .checked_add(key.len())
                    .ok_or_else(|| invalid("attribute bytes overflow"))?;
                self.structure_edit_name(key)?;
                if entries > limits.max_objects || depth >= limits.max_depth {
                    return Err(PdfError::ResourceLimit(
                        "structure attribute entries/depth".into(),
                    ));
                }
                match val {
                    PdfObject::PdfReference(_) | PdfObject::PdfStream(_) => {
                        return Err(PdfError::UnsupportedFeature(
                            "attribute writes require direct data".into(),
                        ));
                    }
                    PdfObject::PdfDict(d) => {
                        pending.extend(d.into_iter().map(|(k, v)| (k.as_str(), v, depth + 1)))
                    }
                    PdfObject::PdfArray(a) => {
                        pending.extend(a.into_iter().map(|v| ("item", v, depth + 1)))
                    }
                    PdfObject::PdfString(s) => {
                        if s.bytes().len() > limits.max_token_bytes {
                            return Err(PdfError::ResourceLimit("attribute string token".into()));
                        }
                        bytes = bytes.saturating_add(s.bytes().len());
                    }
                    PdfObject::PdfNumber(n) => {
                        if !n.value().is_finite() {
                            return Err(invalid("nonfinite attribute number"));
                        }
                        if n.lexeme().len() > limits.max_token_bytes {
                            return Err(PdfError::ResourceLimit("attribute number token".into()));
                        }
                        bytes = bytes.saturating_add(n.lexeme().len());
                    }
                    PdfObject::PdfName(n) => {
                        self.structure_edit_name(n.name())?;
                        bytes = bytes.saturating_add(n.bytes().len());
                    }
                    _ => {}
                }
                if bytes > limits.max_decoded_bytes {
                    return Err(PdfError::ResourceLimit("structure attribute bytes".into()));
                }
            }
            for (key, _) in &attribute.values {
                if matches!(key.as_str(), "O" | "NS")
                    || !seen.insert((attribute.owner.as_str(), key.as_str()))
                {
                    return Err(invalid("reserved or duplicate attribute key"));
                }
            }
            let mut dict = attribute.values.clone();
            dict.try_insert("O".into(), name(&attribute.owner))?;
            attributes.push(PdfObject::PdfDict(dict));
            if revision != 0 {
                attributes.push(number(revision)?);
            }
        }
        dict.remove("A");
        if !attributes.is_empty() {
            dict.try_insert("A".into(), array(attributes)?)?;
        }
        Ok(())
    }
    fn structure_edit_name(&self, value: &str) -> PdfResult<()> {
        if value.is_empty() || value.as_bytes().contains(&0) {
            return Err(invalid("empty/NUL structure name"));
        }
        if value.len() > self.ctx().limits().max_token_bytes {
            return Err(PdfError::ResourceLimit("structure name bytes".into()));
        }
        Ok(())
    }

    /// Create an element, at an index in mixed K order. None denotes the root;
    /// creates the root if absent. Does not claim that all page content is tagged.
    pub fn add_structure_element(
        &self,
        parent: Option<&ObjectHandle>,
        index: usize,
        definition: &StructureElementDefinition,
    ) -> PdfResult<ObjectHandle> {
        self.require_structure_edit()?;
        let parent = parent.map(|h| self.validate_handle(h)).transpose()?;
        self.edit_transaction(|doc| {
            let mut graph = match doc.editable_structure()? {
                Some(graph) => graph,
                None => {
                    if parent.is_some() {
                        return Err(invalid("parent without structure tree"));
                    }
                    let root = doc.try_insert_object(PdfObject::PdfDict(PdfDict::default()))?;
                    let mut dict = PdfDict::default();
                    dict.try_insert("Type".into(), name("StructTreeRoot"))?;
                    Graph {
                        root,
                        dict,
                        kids: Vec::new(),
                        nodes: HashMap::new(),
                        reverse: BTreeMap::new(),
                        reverse_dict: PdfDict::default(),
                    }
                }
            };
            let parent = parent.unwrap_or(graph.root);
            if index > graph.children(parent)?.len() {
                return Err(invalid("structure insertion index"));
            }
            let mut dict = PdfDict::default();
            dict.try_insert("Type".into(), name("StructElem"))?;
            doc.structure_definition(&mut dict, definition)?;
            let id = doc.try_insert_object(PdfObject::PdfDict(dict.clone()))?;
            graph.children(parent)?.insert(index, Kid::Element(id));
            graph.nodes.insert(
                id,
                Node {
                    dict,
                    parent,
                    kids: Vec::new(),
                },
            );
            doc.publish_structure(graph)?;
            doc.object_handle(id)
        })
    }
    /// Replace local role/text/A attributes; existing NS, C, R, Pg and K are retained.
    pub fn update_structure_element(
        &self,
        handle: &ObjectHandle,
        definition: &StructureElementDefinition,
    ) -> PdfResult<()> {
        self.require_structure_edit()?;
        let id = self.validate_handle(handle)?;
        self.edit_transaction(|doc| {
            let mut graph = doc
                .editable_structure()?
                .ok_or_else(|| invalid("missing structure tree"))?;
            graph.node(id)?;
            doc.structure_definition(&mut graph.nodes.get_mut(&id).unwrap().dict, definition)?;
            doc.publish_structure(graph)
        })
    }
    /// Index is measured after detaching, in mixed element/content K order.
    pub fn move_structure_element(
        &self,
        handle: &ObjectHandle,
        parent: Option<&ObjectHandle>,
        index: usize,
    ) -> PdfResult<()> {
        self.require_structure_edit()?;
        let id = self.validate_handle(handle)?;
        let parent = parent.map(|h| self.validate_handle(h)).transpose()?;
        self.edit_transaction(|doc| {
            let mut graph = doc
                .editable_structure()?
                .ok_or_else(|| invalid("missing structure tree"))?;
            let old_parent = graph.node(id)?.parent;
            let parent = parent.unwrap_or(graph.root);
            let mut ancestor = parent;
            while ancestor != graph.root {
                if ancestor == id {
                    return Err(invalid("structure move cycle"));
                }
                ancestor = graph.node(ancestor)?.parent;
            }
            graph
                .children(old_parent)?
                .retain(|k| !matches!(k, Kid::Element(h) if *h == id));
            let kids = graph.children(parent)?;
            if index > kids.len() {
                return Err(invalid("structure insertion index"));
            }
            kids.insert(index, Kid::Element(id));
            graph.nodes.get_mut(&id).unwrap().parent = parent;
            doc.publish_structure(graph)
        })
    }
    /// Detach only the node, not its PDF objects or painted content. This is not
    /// erasure/redaction. PromoteChildren retains and re-owns direct content.
    pub fn remove_structure_element(
        &self,
        handle: &ObjectHandle,
        mode: StructureRemoval,
    ) -> PdfResult<()> {
        self.require_structure_edit()?;
        let id = self.validate_handle(handle)?;
        self.edit_transaction(|doc| {
            let mut graph = doc
                .editable_structure()?
                .ok_or_else(|| invalid("missing structure tree"))?;
            let node = graph.node(id)?;
            if mode == StructureRemoval::EmptyOnly && !node.kids.is_empty() {
                return Err(invalid("structure element is not empty"));
            }
            if node.parent == graph.root
                && node.kids.iter().any(|k| matches!(k, Kid::Content { .. }))
            {
                return Err(invalid("root cannot own promoted content"));
            }
            let node = graph.nodes.remove(&id).unwrap();
            for kid in &node.kids {
                if let Kid::Element(child) = kid {
                    graph.nodes.get_mut(child).unwrap().parent = node.parent;
                }
            }
            let siblings = graph.children(node.parent)?;
            let index = siblings
                .iter()
                .position(|k| matches!(k,Kid::Element(h) if *h == id))
                .ok_or_else(|| invalid("missing parent child"))?;
            siblings.splice(index..index + 1, node.kids);
            doc.publish_structure(graph)
        })
    }

    pub fn set_structure_role_mapping(&self, from: &str, to: Option<&str>) -> PdfResult<()> {
        self.require_structure_edit()?;
        self.structure_edit_name(from)?;
        if let Some(to) = to {
            self.structure_edit_name(to)?;
        }
        self.edit_transaction(|doc| {
            let mut graph = doc
                .editable_structure()?
                .ok_or_else(|| invalid("missing structure tree"))?;
            let mut roles = graph
                .dict
                .get("RoleMap")
                .map(|v| {
                    doc.ctx()
                        .resolve(v)?
                        .into_dict()
                        .ok_or_else(|| invalid("RoleMap dictionary"))
                })
                .transpose()?
                .unwrap_or_default();
            roles.remove(from);
            if let Some(to) = to {
                roles.try_insert(from.into(), name(to))?;
            }
            let mut work = 0usize;
            for (start, _) in &roles {
                let mut seen = HashSet::new();
                let mut next = start.clone();
                while let Some(value) = roles.get(&next) {
                    work = work.saturating_add(1);
                    if work > doc.ctx().limits().max_content_operations {
                        return Err(PdfError::ResourceLimit(
                            "RoleMap cumulative traversal".into(),
                        ));
                    }
                    if !seen.insert(next.clone()) {
                        return Err(invalid("authored RoleMap cycle"));
                    }
                    if seen.len() > doc.ctx().limits().max_objects {
                        return Err(PdfError::ResourceLimit("RoleMap traversal".into()));
                    }
                    let value = doc.ctx().resolve(value)?;
                    let target = value
                        .as_name()
                        .ok_or_else(|| invalid("RoleMap name"))?
                        .name()
                        .to_owned();
                    if seen.contains(&target) {
                        return Err(invalid("authored RoleMap cycle"));
                    }
                    if !roles.contains_key(&target) {
                        break;
                    }
                    next = target;
                }
            }
            graph
                .dict
                .try_insert("RoleMap".into(), PdfObject::PdfDict(roles))?;
            doc.publish_structure(graph)
        })
    }

    /// Wrap a nonempty, unmarked, balanced range of original page operations.
    /// The stamp must be current. Form programs are not modified: a selected Do
    /// belongs to the page, while existing structural Form MCIDs stay separate.
    /// kid_index is the insertion position in the element's mixed K order.
    pub fn mark_structure_content(
        &self,
        element: &ObjectHandle,
        page_index: u32,
        stamp: &DocumentStamp,
        operations: Range<usize>,
        kid_index: usize,
        actual_text: Option<&str>,
    ) -> PdfResult<StructureMark> {
        self.require_structure_edit()?;
        let element_id = self.validate_handle(element)?;
        if !self.is_stamp_current(stamp) {
            return Err(invalid("stale/foreign content selection"));
        }
        self.edit_transaction(|doc| {
            let mut graph = doc
                .editable_structure()?
                .ok_or_else(|| invalid("missing structure tree"))?;
            if kid_index > graph.node(element_id)?.kids.len() {
                return Err(invalid("content insertion index"));
            }
            let mut page = doc.get_page(page_index)?;
            // Validate actual interpreter scope/resources before publishing.
            let report = page.extract_marked_text(Default::default())?;
            if !report.diagnostics.is_empty() {
                return Err(invalid("content selection has diagnostics"));
            }
            let program = page.content_program()?;
            let range = selected_bytes(&program, operations, doc.ctx().limits().max_depth)?;
            let location = doc.page_location(page_index)?;
            let mut page_dict = location.page_dict;
            let key = if let Some(key) = page_dict.get("StructParents") {
                doc.ctx()
                    .resolve(key)?
                    .as_number()
                    .ok_or_else(|| invalid("StructParents number"))?
                    .as_u32_checked()?
            } else {
                let min = graph
                    .reverse
                    .keys()
                    .next_back()
                    .copied()
                    .map(|k| {
                        k.checked_add(1)
                            .ok_or_else(|| invalid("ParentTree key overflow"))
                    })
                    .transpose()?
                    .unwrap_or(0);
                let next = graph
                    .dict
                    .get("ParentTreeNextKey")
                    .map(|v| {
                        doc.ctx()
                            .resolve(v)?
                            .as_number()
                            .ok_or_else(|| invalid("ParentTreeNextKey number"))?
                            .as_u32_checked()
                    })
                    .transpose()?
                    .unwrap_or(0)
                    .max(min);
                graph.reverse.insert(next, array([])?);
                page_dict.try_insert("StructParents".into(), number(next)?)?;
                next
            };
            let previous = doc.ctx().resolve(
                graph
                    .reverse
                    .get(&key)
                    .ok_or_else(|| invalid("ParentTree key missing"))?,
            )?;
            let slots = previous
                .as_array()
                .ok_or_else(|| invalid("ParentTree array"))?;
            if slots.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("MCID slots".into()));
            }
            let mcid = u32::try_from(slots.len()).map_err(|_| invalid("MCID overflow"))?;
            let mut values = slots.clone();
            values.try_add_obj(reference(element_id))?;
            graph.reverse.insert(key, PdfObject::PdfArray(values));
            let mut prefix = format!("\n/Span << /MCID {mcid}").into_bytes();
            if let Some(text) = actual_text {
                let text = PdfString::from_text_with_limits(text, &doc.ctx().limits())?;
                prefix.extend_from_slice(b" /ActualText <");
                const HEX: &[u8] = b"0123456789ABCDEF";
                for byte in text.bytes() {
                    prefix.push(HEX[(byte >> 4) as usize]);
                    prefix.push(HEX[(byte & 15) as usize]);
                }
                prefix.push(b'>');
            }
            prefix.extend_from_slice(b" >> BDC\n");
            let source = program.source_bytes();
            let length = source
                .len()
                .checked_add(prefix.len())
                .and_then(|n| n.checked_add(5))
                .ok_or_else(|| invalid("marked bytes overflow"))?;
            if length > doc.ctx().limits().max_decoded_bytes
                || length > doc.ctx().limits().max_raw_stream_bytes
            {
                return Err(PdfError::ResourceLimit("marked page bytes".into()));
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| PdfError::ResourceLimit("marked page allocation".into()))?;
            bytes.extend_from_slice(&source[..range.start]);
            bytes.extend_from_slice(&prefix);
            bytes.extend_from_slice(&source[range.clone()]);
            bytes.extend_from_slice(b"\nEMC\n");
            bytes.extend_from_slice(&source[range.end..]);
            ContentProgram::from_decoded_bytes(bytes.clone(), doc.ctx().options())?;
            let stream = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), bytes)?,
            ))?;
            page_dict.try_insert("Contents".into(), reference(stream))?;
            doc.update_object(location.page_id, PdfObject::PdfDict(page_dict))?;
            let page_handle = doc.object_handle(location.page_id)?;
            let mut mcr = PdfDict::default();
            mcr.try_insert("Type".into(), name("MCR"))?;
            mcr.try_insert("Pg".into(), reference(location.page_id))?;
            mcr.try_insert("MCID".into(), number(mcid)?)?;
            graph.nodes.get_mut(&element_id).unwrap().kids.insert(
                kid_index,
                Kid::Content {
                    raw: PdfObject::PdfDict(mcr),
                    association: StructureAssociation::MarkedContent {
                        element: element.clone(),
                        page: page_handle.clone(),
                        domain: StructureContentDomain::Page(page_handle.clone()),
                        mcid,
                    },
                },
            );
            doc.publish_structure(graph)?;
            let report = doc
                .get_page(page_index)?
                .extract_marked_text(Default::default())?;
            if !report.diagnostics.is_empty() {
                return Err(invalid("marked result has diagnostics"));
            }
            Ok(StructureMark {
                stamp: doc.stamp(),
                element: element.clone(),
                page: page_handle,
                mcid,
            })
        })
    }

    fn publish_structure(&self, mut graph: Graph) -> PdfResult<()> {
        // ParentTree values may be shared/indirect. Always write fresh arrays.
        let mut replacements: BTreeMap<u32, PdfObject> = BTreeMap::new();
        let mut arrays: BTreeMap<u32, Vec<PdfObject>> = BTreeMap::new();
        let mut slots = 0usize;
        for (id, node) in &graph.nodes {
            for kid in &node.kids {
                let Kid::Content { association, .. } = kid else {
                    continue;
                };
                match association {
                    StructureAssociation::MarkedContent { domain, mcid, .. } => {
                        let (StructureContentDomain::Page(h) | StructureContentDomain::Form(h)) =
                            domain;
                        let value = self.resolve_object(h.object_id())?;
                        let dict = value
                            .as_dict()
                            .or_else(|| value.as_stream().map(|s| s.dict()))
                            .ok_or_else(|| invalid("content domain dictionary"))?;
                        let key = dict
                            .get("StructParents")
                            .ok_or_else(|| invalid("StructParents missing"))?;
                        let key = self
                            .ctx()
                            .resolve(key)?
                            .as_number()
                            .ok_or_else(|| invalid("StructParents number"))?
                            .as_u32_checked()?;
                        if let std::collections::btree_map::Entry::Vacant(entry) = arrays.entry(key)
                        {
                            let old = graph
                                .reverse
                                .get(&key)
                                .ok_or_else(|| invalid("ParentTree key missing"))?;
                            let old = self.ctx().resolve(old)?;
                            let old = old.as_array().ok_or_else(|| invalid("ParentTree array"))?;
                            slots = slots.saturating_add(old.len());
                            if slots > self.ctx().limits().max_objects {
                                return Err(PdfError::ResourceLimit("ParentTree slots".into()));
                            }
                            entry.insert(old.into_iter().cloned().collect());
                        }
                        *arrays
                            .get_mut(&key)
                            .unwrap()
                            .get_mut(*mcid as usize)
                            .ok_or_else(|| invalid("ParentTree slot missing"))? = reference(*id);
                    }
                    StructureAssociation::ObjectReference { object, .. } => {
                        let value = self.resolve_object(object.object_id())?;
                        let dict = value
                            .as_dict()
                            .or_else(|| value.as_stream().map(|s| s.dict()))
                            .ok_or_else(|| invalid("OBJR dictionary"))?;
                        let key = self
                            .ctx()
                            .resolve(
                                dict.get("StructParent")
                                    .ok_or_else(|| invalid("StructParent missing"))?,
                            )?
                            .as_number()
                            .ok_or_else(|| invalid("StructParent number"))?
                            .as_u32_checked()?;
                        if replacements
                            .insert(key, reference(*id))
                            .is_some_and(|old| old != reference(*id))
                        {
                            return Err(invalid("conflicting OBJR ownership"));
                        }
                    }
                }
            }
        }
        for (key, values) in arrays {
            replacements.insert(key, array(values)?);
        }
        graph.reverse.extend(replacements);
        let entries: Vec<_> = graph
            .reverse
            .into_iter()
            .map(|(k, v)| (TreeKey::Number(k), v))
            .collect();
        graph.dict.try_insert(
            "ParentTree".into(),
            PdfObject::PdfDict(self.build_tree(TreeKind::Number, &entries, graph.reverse_dict)?),
        )?;
        let next = entries
            .last()
            .map(|(k, _)| {
                let TreeKey::Number(k) = k else {
                    unreachable!()
                };
                k.checked_add(1)
                    .ok_or_else(|| invalid("ParentTree key overflow"))
            })
            .transpose()?
            .unwrap_or(0);
        if let Some(old) = graph.dict.get("ParentTreeNextKey") {
            let old = self
                .ctx()
                .resolve(old)?
                .as_number()
                .ok_or_else(|| invalid("ParentTreeNextKey number"))?
                .as_u32_checked()?;
            graph
                .dict
                .try_insert("ParentTreeNextKey".into(), number(next.max(old))?)?;
        } else {
            graph
                .dict
                .try_insert("ParentTreeNextKey".into(), number(next)?)?;
        }
        fn kids(kids: Vec<Kid>) -> PdfResult<PdfObject> {
            array(kids.into_iter().map(|k| match k {
                Kid::Element(id) => reference(id),
                Kid::Content { raw, .. } => raw,
            }))
        }
        graph.dict.try_insert("K".into(), kids(graph.kids)?)?;
        for (id, mut node) in graph.nodes {
            node.dict.try_insert("P".into(), reference(node.parent))?;
            node.dict.try_insert("K".into(), kids(node.kids)?)?;
            self.update_object(id, PdfObject::PdfDict(node.dict))?;
        }
        self.update_object(graph.root, PdfObject::PdfDict(graph.dict))?;
        let mut catalog = self.ctx().get_root_owned()?;
        catalog.try_insert("StructTreeRoot".into(), reference(graph.root))?;
        self.update_object(
            indirect(
                self.ctx()
                    .trailer()
                    .get("Root")
                    .ok_or_else(|| invalid("Catalog reference"))?,
            )?,
            PdfObject::PdfDict(catalog),
        )?;
        self.structure_associations()?
            .ok_or_else(|| invalid("published structure disappeared"))?;
        Ok(())
    }
}

fn selected_bytes(
    program: &ContentProgram,
    selection: Range<usize>,
    max_depth: usize,
) -> PdfResult<Range<usize>> {
    let ops = program.operations();
    if selection.start >= selection.end || selection.end > ops.len() {
        return Err(invalid("empty/out-of-range operation selection"));
    }
    let start = ops[selection.start].lexical_range.start;
    let end = ops[selection.end - 1].lexical_range.end;
    if !program
        .stream_ranges()
        .iter()
        .any(|r| start >= r.start && end <= r.end)
    {
        return Err(PdfError::UnsupportedFeature(
            "marked selection across Contents streams".into(),
        ));
    }
    let mut graphics = 0usize;
    let mut text = false;
    let mut marks = 0usize;
    let mut path = false;
    let mut initial = None;
    for (index, op) in ops.iter().enumerate() {
        if index == selection.start {
            if marks != 0 || path {
                return Err(invalid("selection starts inside marked content/path"));
            }
            initial = Some((graphics, text));
        }
        let selected = selection.contains(&index);
        let op = &op.operator;
        match op.name() {
            "BMC" | "BDC" => {
                if selected {
                    return Err(invalid("selection already contains marked content"));
                }
                marks = marks
                    .checked_add(1)
                    .ok_or_else(|| invalid("marked depth overflow"))?;
            }
            "EMC" => {
                if selected {
                    return Err(invalid("selection crosses marked content"));
                }
                marks = marks
                    .checked_sub(1)
                    .ok_or_else(|| invalid("unbalanced EMC"))?;
            }
            "q" => {
                graphics = graphics
                    .checked_add(1)
                    .ok_or_else(|| invalid("graphics depth overflow"))?
            }
            "Q" => {
                graphics = graphics
                    .checked_sub(1)
                    .ok_or_else(|| invalid("unbalanced Q"))?;
                if selected && graphics < initial.unwrap().0 {
                    return Err(invalid("selection crosses enclosing graphics state"));
                }
            }
            "BT" => {
                if text {
                    return Err(invalid("nested BT"));
                }
                text = true;
            }
            "ET" => {
                if !text {
                    return Err(invalid("unbalanced ET"));
                }
                if selected && initial.unwrap().1 {
                    return Err(invalid("selection crosses enclosing text object"));
                }
                text = false;
            }
            "m" | "l" | "c" | "v" | "y" | "h" | "re" | "W" | "W*" => path = true,
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => path = false,
            _ => {}
        }
        if marks > max_depth || graphics > max_depth {
            return Err(PdfError::ResourceLimit("mark selection depth".into()));
        }
        if index + 1 == selection.end && (initial != Some((graphics, text)) || marks != 0 || path) {
            return Err(invalid("selection is not a complete marked-content scope"));
        }
    }
    if graphics != 0 || text || marks != 0 || path {
        return Err(invalid("unbalanced source program"));
    }
    Ok(start..end)
}
