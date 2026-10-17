//! Read-only hierarchy snapshots and explicit PDF 1.x content-ownership checks.
//! The shallow snapshot keeps content items and reverse indexes raw; callers
//! opt into `structure_associations()` for the supported ownership validation.
use super::{
    PdfDocument,
    identity::{DocumentStamp, ObjectHandle},
};
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek},
};

mod associations;
pub use associations::{StructureAssociation, StructureAssociations, StructureContentDomain};
mod edit;
mod semantics;
mod text;
pub use edit::{
    StructureAttributeWrite, StructureElementDefinition, StructureMark, StructureRemoval,
};
pub use semantics::{
    StructureAttribute, StructureAttributeBinding, StructureAttributeClass,
    StructureAttributeLookup, StructureAttributeOwner, StructureAttributeSource, StructureClassUse,
    StructureNamespace, StructureRoleMapping, StructureRoleResolution, StructureRoleStatus,
    StructureRoleTarget,
};
pub use text::{StructuredTextReport, StructuredTextSpan};

#[derive(Debug, Clone, PartialEq)]
pub enum StructureKid {
    Element(ObjectHandle),
    /// An integer MCID or raw MCR/OBJR dictionary/reference. Its page/stream,
    /// ParentTree and actual content membership have NOT yet been validated.
    UnvalidatedContent(PdfObject),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructureElement {
    pub handle: ObjectHandle,
    pub parent: ObjectHandle,
    pub role: String,
    /// Legacy root RoleMap endpoint for elements without NS. None means a
    /// cyclic association or an explicitly namespaced role; use resolved_role
    /// to distinguish them. Not a claim of standard-role validity.
    pub mapped_role: Option<String>,
    /// Explicit namespace only; never inherited from an ancestor.
    pub namespace: Option<ObjectHandle>,
    /// Namespace-qualified mapping endpoint, not a standard-role/Tagged verdict.
    pub resolved_role: StructureRoleResolution,
    pub revision: u32,
    pub classes: Vec<StructureClassUse>,
    /// Class bindings first, then direct A bindings, preserving source order.
    pub attribute_bindings: Vec<StructureAttributeBinding>,
    /// PDF 2.0 defines later-entry precedence; PDF 1.x arrays are unordered.
    pub attributes_use_array_order: bool,
    pub page: Option<ObjectHandle>,
    pub title: Option<String>,
    pub language: Option<String>,
    pub alternate_text: Option<String>,
    pub actual_text: Option<String>,
    pub children: Vec<StructureKid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructureTree {
    pub stamp: DocumentStamp,
    pub root: ObjectHandle,
    pub children: Vec<StructureKid>,
    /// Preorder, preserving the declared /K order, not page paint order.
    pub elements: Vec<StructureElement>,
    pub namespaces: Vec<StructureNamespace>,
    pub attribute_classes: Vec<StructureAttributeClass>,
    /// Raw /ParentTree; use structure_associations() for supported validation.
    pub unvalidated_parent_tree: Option<PdfObject>,
}

fn invalid(message: &str) -> PdfError {
    PdfError::DocumentError(format!("structure tree: {message}"))
}
fn indirect(object: &PdfObject) -> PdfResult<ObjectId> {
    object
        .as_reference()
        .map(PdfReference::id)
        .ok_or_else(|| invalid("structure ownership must be indirect"))
}

fn element_error(id: ObjectId, error: PdfError) -> PdfError {
    match error {
        PdfError::DocumentError(message) => {
            PdfError::DocumentError(format!("{message} (structure element {id:?})"))
        }
        other => other,
    }
}

struct ReadState {
    seen: HashSet<ObjectId>,
    entries: usize,
    text_bytes: usize,
    roles: HashMap<String, String>,
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Inspect structure hierarchy without editing objects or executing
    /// actions. Missing trees return None. Content/ParentTree coherence, content
    /// extraction and full Tagged conformance are NOT validated here. Attributes
    /// have local owner/class precedence only, not layout/default inheritance.
    pub fn structure_tree(&self) -> PdfResult<Option<StructureTree>> {
        let root = self.ctx().get_root_owned()?;
        let Some(value) = root
            .get("StructTreeRoot")
            .filter(|v| !matches!(v, PdfObject::PdfNull))
        else {
            return Ok(None);
        };
        let root_id = indirect(value)?;
        let dict = self.structure_dict(root_id)?;
        if self.structure_name(&dict, "Type")?.as_deref() != Some("StructTreeRoot") {
            return Err(invalid("expected StructTreeRoot Type"));
        }
        let mut state = ReadState {
            seen: HashSet::from([root_id]),
            entries: 1,
            text_bytes: 0,
            roles: HashMap::new(),
        };
        if state.entries > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("structure entries".into()));
        }
        if let Some(value) = dict.get("RoleMap") {
            let roles = self
                .ctx()
                .resolve(value)?
                .into_dict()
                .ok_or_else(|| invalid("RoleMap dictionary"))?;
            for (name, value) in &roles {
                let value = self.ctx().resolve(value)?;
                let target = value
                    .as_name()
                    .ok_or_else(|| invalid("RoleMap target must be a name"))?
                    .name();
                if name.len() > self.ctx().limits().max_token_bytes
                    || target.len() > self.ctx().limits().max_token_bytes
                {
                    return Err(PdfError::ResourceLimit("structure role token bytes".into()));
                }
                self.structure_charge(&mut state, name.len().saturating_add(target.len()))?;
                if state.roles.len() >= self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("structure role mappings".into()));
                }
                state.roles.insert(name.clone(), target.to_string());
            }
            for role in state.roles.keys() {
                self.structure_role(role, &state)?;
            }
        }
        let mut semantics = semantics::SemanticReader::new(self, &dict, &mut state)?;
        let pages: HashSet<_> = self
            .collect_page_entries()?
            .into_iter()
            .map(|p| p.page_id)
            .collect();
        let children = self.structure_kids(&dict, &mut state)?;
        if children
            .iter()
            .any(|k| !matches!(k, StructureKid::Element(_)))
        {
            return Err(invalid("root K must contain structure elements"));
        }
        let mut pending = Vec::new();
        for kid in children.iter().rev() {
            if let StructureKid::Element(h) = kid {
                pending.push((h.object_id(), root_id, 0usize));
            }
        }
        let mut elements = Vec::new();
        while let Some((id, parent, depth)) = pending.pop() {
            if depth >= self.ctx().limits().max_depth {
                return Err(PdfError::ResourceLimit("structure hierarchy depth".into()));
            }
            let node = self.structure_dict(id)?;
            if node.get("P").map(indirect).transpose()? != Some(parent) {
                return Err(invalid("P/K ownership mismatch"));
            }
            let role = self
                .structure_name(&node, "S")?
                .filter(|s| !s.is_empty())
                .ok_or_else(|| invalid("structure S missing"))?;
            self.structure_charge(&mut state, role.len())?;
            let namespace = semantics
                .element_namespace(self, &node, &mut state)
                .map_err(|e| element_error(id, e))?;
            let resolved_role = semantics
                .resolve_role(self, &role, namespace.as_ref(), &mut state)
                .map_err(|e| element_error(id, e))?;
            let mapped_role = if namespace.is_none() {
                self.structure_role(&role, &state)?
            } else {
                None
            };
            self.structure_charge(&mut state, mapped_role.as_ref().map_or(0, String::len))?;
            self.structure_charge(
                &mut state,
                resolved_role
                    .name
                    .len()
                    .saturating_add(resolved_role.namespace_uri.len()),
            )?;
            let (revision, classes, attribute_bindings) = semantics
                .element_attributes(self, &node, &mut state)
                .map_err(|e| element_error(id, e))?;
            let page = node
                .get("Pg")
                .map(|v| {
                    let id = indirect(v)?;
                    if !pages.contains(&id) {
                        return Err(invalid("Pg is not a document page"));
                    }
                    self.object_handle(id)
                })
                .transpose()?;
            let children = self.structure_kids(&node, &mut state)?;
            for kid in children.iter().rev() {
                if let StructureKid::Element(h) = kid {
                    pending.push((h.object_id(), id, depth + 1));
                }
            }
            elements.push(StructureElement {
                handle: self.object_handle(id)?,
                parent: self.object_handle(parent)?,
                role,
                mapped_role,
                namespace,
                resolved_role,
                revision,
                classes,
                attribute_bindings,
                attributes_use_array_order: semantics.pdf2,
                page,
                title: self.structure_text(&node, "T", &mut state)?,
                language: self.structure_text(&node, "Lang", &mut state)?,
                alternate_text: self.structure_text(&node, "Alt", &mut state)?,
                actual_text: self.structure_text(&node, "ActualText", &mut state)?,
                children,
            });
        }
        Ok(Some(StructureTree {
            stamp: self.stamp(),
            root: self.object_handle(root_id)?,
            children,
            elements,
            namespaces: semantics.namespaces,
            attribute_classes: semantics.classes.into_values().collect(),
            unvalidated_parent_tree: dict.get("ParentTree").cloned(),
        }))
    }

    fn structure_dict(&self, id: ObjectId) -> PdfResult<PdfDict> {
        self.resolve_object(id)?
            .into_dict()
            .ok_or_else(|| invalid("structure dictionary"))
    }
    fn structure_name(&self, dict: &PdfDict, key: &str) -> PdfResult<Option<String>> {
        dict.get(key)
            .map(|v| {
                let value = self.ctx().resolve(v)?;
                let name = value.as_name().ok_or_else(|| invalid("expected name"))?;
                if name.bytes().len() > self.ctx().limits().max_token_bytes {
                    return Err(PdfError::ResourceLimit("structure name bytes".into()));
                }
                Ok(name.name().to_string())
            })
            .transpose()
    }
    fn structure_charge(&self, state: &mut ReadState, size: usize) -> PdfResult<()> {
        state.text_bytes = state
            .text_bytes
            .checked_add(size)
            .filter(|n| *n <= self.ctx().limits().max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("structure cumulative text bytes".into()))?;
        Ok(())
    }
    fn structure_text(
        &self,
        dict: &PdfDict,
        key: &str,
        state: &mut ReadState,
    ) -> PdfResult<Option<String>> {
        dict.get(key)
            .map(|v| {
                let value = self.ctx().resolve(v)?;
                let text = value
                    .as_string()
                    .ok_or_else(|| invalid("expected text string"))?
                    .text_content_with_limits(&self.ctx().limits())?;
                self.structure_charge(state, text.len())?;
                Ok(text)
            })
            .transpose()
    }
    fn structure_role(&self, role: &str, state: &ReadState) -> PdfResult<Option<String>> {
        let mut current = role;
        let mut seen = HashSet::new();
        while let Some(next) = state.roles.get(current) {
            if !seen.insert(current) {
                return Ok(None);
            }
            if seen.len() > self.ctx().limits().max_depth {
                return Err(PdfError::ResourceLimit("structure RoleMap depth".into()));
            }
            current = next;
        }
        Ok(Some(current.to_string()))
    }
    fn structure_kids(
        &self,
        dict: &PdfDict,
        state: &mut ReadState,
    ) -> PdfResult<Vec<StructureKid>> {
        let Some(value) = dict.get("K").filter(|v| !matches!(v, PdfObject::PdfNull)) else {
            return Ok(Vec::new());
        };
        let resolved = self.ctx().resolve(value)?;
        let items = if let PdfObject::PdfArray(array) = &resolved {
            if array.len()
                > self
                    .ctx()
                    .limits()
                    .max_objects
                    .saturating_sub(state.entries)
            {
                return Err(PdfError::ResourceLimit("structure children".into()));
            }
            array.into_iter().cloned().collect::<Vec<_>>()
        } else {
            vec![value.clone()]
        };
        let mut kids = Vec::new();
        for item in items {
            state.entries = state
                .entries
                .checked_add(1)
                .filter(|n| *n <= self.ctx().limits().max_objects)
                .ok_or_else(|| PdfError::ResourceLimit("structure entries".into()))?;
            let value = self.ctx().resolve(&item)?;
            match value {
                PdfObject::PdfNumber(n) => {
                    n.as_u32_checked()?;
                    kids.push(StructureKid::UnvalidatedContent(item));
                }
                PdfObject::PdfDict(dict) => match self.structure_name(&dict, "Type")?.as_deref() {
                    Some("StructElem") | None => {
                        let id = indirect(&item)?;
                        if !state.seen.insert(id) {
                            return Err(invalid("cycle or shared structure element"));
                        }
                        kids.push(StructureKid::Element(self.object_handle(id)?));
                    }
                    Some("MCR" | "OBJR") => kids.push(StructureKid::UnvalidatedContent(item)),
                    _ => {
                        return Err(PdfError::UnsupportedFeature(
                            "unknown structure child dictionary".into(),
                        ));
                    }
                },
                _ => return Err(invalid("unsupported K value")),
            }
        }
        Ok(kids)
    }
}
