//! Namespace-qualified mapping and local attribute attachment semantics.
//! Unknown value objects and stream payloads are retained, never executed.
use super::{ReadState, StructureElement, indirect, invalid};
use crate::{
    document::{PdfDocument, identity::ObjectHandle, save_options::PdfVersion},
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Read, Seek},
};

const DEFAULT_NAMESPACE: &str = "http://iso.org/pdf/ssn";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureRoleTarget {
    pub name: String,
    /// None denotes the default standard namespace, not the root RoleMap.
    pub namespace: Option<ObjectHandle>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureRoleMapping {
    pub from: String,
    pub to: StructureRoleTarget,
}
#[derive(Debug, Clone, PartialEq)]
pub struct StructureNamespace {
    pub handle: ObjectHandle,
    pub uri: String,
    /// Includes transitive mapping/attribute targets not listed in Namespaces.
    pub listed: bool,
    pub schema: Option<PdfObject>,
    pub role_map: Vec<StructureRoleMapping>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureRoleStatus {
    Endpoint,
    Cycle,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureRoleResolution {
    pub name: String,
    pub namespace: Option<ObjectHandle>,
    pub namespace_uri: String,
    pub status: StructureRoleStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StructureAttributeOwner {
    Name(String),
    /// NSO owners are identified by namespace URI, not dictionary object ID.
    Namespace(String),
}
#[derive(Debug, Clone, PartialEq)]
pub struct StructureAttribute {
    pub owner: StructureAttributeOwner,
    pub namespace: Option<ObjectHandle>,
    /// Raw values, scoped to this document; O/NS and stream machinery excluded.
    pub values: PdfDict,
    /// Opaque owner-specific payload. Never decoded to guess attribute values.
    pub stream: Option<ObjectHandle>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct StructureAttributeClass {
    pub name: String,
    pub attributes: Vec<StructureAttribute>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureClassUse {
    pub name: String,
    pub revision: u32,
    pub defined: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureAttributeSource {
    Direct,
    Class(String),
}
#[derive(Debug, Clone, PartialEq)]
pub struct StructureAttributeBinding {
    pub attribute: StructureAttribute,
    pub revision: u32,
    pub source: StructureAttributeSource,
}
/// Local attachment result only: no inherited/default layout attributes.
#[derive(Debug, Clone, PartialEq)]
pub enum StructureAttributeLookup<'a> {
    Missing,
    Value {
        value: &'a PdfObject,
        binding: &'a StructureAttributeBinding,
    },
    /// PDF 1.x defines no order between duplicate owner/key attachments.
    Ambiguous(Vec<&'a StructureAttributeBinding>),
    /// A stream might define the requested value in owner-specific data.
    Opaque(Vec<&'a StructureAttributeBinding>),
}
impl StructureElement {
    pub fn attribute(
        &self,
        owner: &StructureAttributeOwner,
        name: &str,
    ) -> StructureAttributeLookup<'_> {
        let mut candidates: Vec<_> = self
            .attribute_bindings
            .iter()
            .filter(|b| {
                &b.attribute.owner == owner
                    && (b.attribute.stream.is_some() || b.attribute.values.contains_key(name))
            })
            .collect();
        if candidates
            .iter()
            .any(|b| b.source == StructureAttributeSource::Direct)
        {
            candidates.retain(|b| b.source == StructureAttributeSource::Direct);
        }
        if self.attributes_use_array_order {
            // Opaque streams remain conservative even if a later dictionary
            // supplies a value: their owner-specific merge rules are unknown.
            if !candidates.iter().any(|b| b.attribute.stream.is_some()) {
                if let Some(last) = candidates.pop() {
                    candidates = vec![last];
                }
            }
        }
        if candidates.iter().any(|b| b.attribute.stream.is_some()) {
            return StructureAttributeLookup::Opaque(candidates);
        }
        match candidates.as_slice() {
            [] => StructureAttributeLookup::Missing,
            [binding] => StructureAttributeLookup::Value {
                value: binding.attribute.values.get(name).unwrap(),
                binding,
            },
            _ => StructureAttributeLookup::Ambiguous(candidates),
        }
    }
}

pub(super) struct SemanticReader {
    pub pdf2: bool,
    pub namespaces: Vec<StructureNamespace>,
    namespace_ids: HashMap<ObjectId, usize>,
    namespace_uris: HashSet<String>,
    pub classes: BTreeMap<String, StructureAttributeClass>,
}
impl SemanticReader {
    pub fn new<R: Read + Seek>(
        doc: &PdfDocument<R>,
        root: &PdfDict,
        state: &mut ReadState,
    ) -> PdfResult<Self> {
        let mut reader = Self {
            pdf2: doc.effective_version()? >= PdfVersion::V2_0,
            namespaces: Vec::new(),
            namespace_ids: HashMap::new(),
            namespace_uris: HashSet::new(),
            classes: BTreeMap::new(),
        };
        if let Some(value) = root.get("Namespaces") {
            reader.require_pdf2()?;
            let value = doc.ctx().resolve(value)?;
            let array = value
                .as_array()
                .ok_or_else(|| invalid("Namespaces array"))?;
            reader.entries(doc, state, array.len())?;
            for item in array {
                reader.load_namespace(doc, indirect(item)?, true, state)?;
            }
        }
        if let Some(value) = root.get("ClassMap") {
            let value = doc.ctx().resolve(value)?;
            let dict = value
                .as_dict()
                .ok_or_else(|| invalid("ClassMap dictionary"))?;
            for (name, value) in dict {
                reader.entries(doc, state, 1)?;
                reader.token(doc, state, name)?;
                let attributes = reader
                    .attribute_objects(doc, value, state, false)?
                    .into_iter()
                    .map(|(attribute, _)| attribute)
                    .collect();
                reader.classes.insert(
                    name.clone(),
                    StructureAttributeClass {
                        name: name.clone(),
                        attributes,
                    },
                );
            }
        }
        Ok(reader)
    }
    fn require_pdf2(&self) -> PdfResult<()> {
        if !self.pdf2 {
            return Err(PdfError::UnsupportedFeature(
                "structure namespaces require effective PDF 2.0".into(),
            ));
        }
        Ok(())
    }
    fn entries<R: Read + Seek>(
        &self,
        doc: &PdfDocument<R>,
        state: &mut ReadState,
        n: usize,
    ) -> PdfResult<()> {
        state.entries = state
            .entries
            .checked_add(n)
            .filter(|v| *v <= doc.ctx().limits().max_objects)
            .ok_or_else(|| PdfError::ResourceLimit("structure semantic entries".into()))?;
        Ok(())
    }
    fn token<R: Read + Seek>(
        &self,
        doc: &PdfDocument<R>,
        state: &mut ReadState,
        name: &str,
    ) -> PdfResult<()> {
        if name.len() > doc.ctx().limits().max_token_bytes {
            return Err(PdfError::ResourceLimit(
                "structure semantic name bytes".into(),
            ));
        }
        doc.structure_charge(state, name.len())
    }
    fn name<R: Read + Seek>(
        &self,
        doc: &PdfDocument<R>,
        value: &PdfObject,
        state: &mut ReadState,
    ) -> PdfResult<String> {
        let value = doc.ctx().resolve(value)?;
        let name = value
            .as_name()
            .ok_or_else(|| invalid("semantic value must be a name"))?
            .name();
        self.token(doc, state, name)?;
        Ok(name.to_string())
    }
    fn integer<R: Read + Seek>(&self, doc: &PdfDocument<R>, value: &PdfObject) -> PdfResult<u32> {
        doc.ctx()
            .resolve(value)?
            .as_number()
            .ok_or_else(|| invalid("attribute revision integer"))?
            .as_u32_checked()
            .map_err(|_| invalid("attribute revision must be a nonnegative integer"))
    }
    // Check inline value size without dereferencing unknown extensions or
    // stream payloads. References remain document-scoped opaque values.
    fn raw_value<R: Read + Seek>(
        &self,
        doc: &PdfDocument<R>,
        value: &PdfObject,
        state: &mut ReadState,
    ) -> PdfResult<()> {
        let mut pending = vec![(value, 0usize)];
        while let Some((value, depth)) = pending.pop() {
            if depth >= doc.ctx().limits().max_depth {
                return Err(PdfError::ResourceLimit(
                    "structure attribute value depth".into(),
                ));
            }
            self.entries(doc, state, 1)?;
            match value {
                PdfObject::PdfDict(dict) => {
                    if dict.into_iter().len()
                        > doc
                            .ctx()
                            .limits()
                            .max_objects
                            .saturating_sub(state.entries)
                            .saturating_sub(pending.len())
                    {
                        return Err(PdfError::ResourceLimit(
                            "structure attribute dictionary entries".into(),
                        ));
                    }
                    for (key, value) in dict {
                        self.token(doc, state, key)?;
                        pending.push((value, depth + 1));
                    }
                }
                PdfObject::PdfArray(array) => {
                    if array.len()
                        > doc
                            .ctx()
                            .limits()
                            .max_objects
                            .saturating_sub(state.entries)
                            .saturating_sub(pending.len())
                    {
                        return Err(PdfError::ResourceLimit(
                            "structure attribute array entries".into(),
                        ));
                    }
                    pending.extend(array.into_iter().map(|v| (v, depth + 1)));
                }
                PdfObject::PdfName(name) => self.token(doc, state, name.name())?,
                PdfObject::PdfString(s) => {
                    if s.bytes().len() > doc.ctx().limits().max_token_bytes {
                        return Err(PdfError::ResourceLimit("structure raw string bytes".into()));
                    }
                    doc.structure_charge(state, s.bytes().len())?;
                }
                PdfObject::PdfStream(_) => {
                    return Err(invalid("nested attribute streams must be indirect"));
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn load_namespace<R: Read + Seek>(
        &mut self,
        doc: &PdfDocument<R>,
        id: ObjectId,
        listed: bool,
        state: &mut ReadState,
    ) -> PdfResult<()> {
        self.require_pdf2()?;
        if let Some(index) = self.namespace_ids.get(&id) {
            self.namespaces[*index].listed |= listed;
            return Ok(());
        }
        let mut pending = vec![(id, listed, 0usize)];
        while let Some((id, listed, depth)) = pending.pop() {
            if let Some(index) = self.namespace_ids.get(&id) {
                self.namespaces[*index].listed |= listed;
                continue;
            }
            if depth >= doc.ctx().limits().max_depth {
                return Err(PdfError::ResourceLimit(
                    "structure namespace graph depth".into(),
                ));
            }
            self.entries(doc, state, 1)?;
            let dict = doc.structure_dict(id)?;
            if doc
                .structure_name(&dict, "Type")?
                .is_some_and(|name| name != "Namespace")
            {
                return Err(invalid("namespace Type must be Namespace when present"));
            }
            let uri = doc
                .structure_text(&dict, "NS", state)?
                .filter(|s| !s.is_empty())
                .ok_or_else(|| invalid("namespace NS text string required"))?;
            if !self.namespace_uris.insert(uri.clone()) {
                return Err(PdfError::UnsupportedFeature(
                    "multiple namespace dictionaries for one URI".into(),
                ));
            }
            let mut role_map = Vec::new();
            if let Some(value) = dict.get("RoleMapNS") {
                let value = doc.ctx().resolve(value)?;
                let roles = value
                    .as_dict()
                    .ok_or_else(|| invalid("RoleMapNS dictionary"))?;
                for (from, value) in roles {
                    self.entries(doc, state, 1)?;
                    self.token(doc, state, from)?;
                    let value = doc.ctx().resolve(value)?;
                    let (name, namespace) = if let Some(array) = value.as_array() {
                        if array.len() != 2 {
                            return Err(invalid("RoleMapNS target array needs name and namespace"));
                        }
                        let target = indirect(array.get(1).unwrap())?;
                        pending.push((target, false, depth + 1));
                        (
                            self.name(doc, array.get(0).unwrap(), state)?,
                            Some(doc.object_handle(target)?),
                        )
                    } else {
                        (self.name(doc, &value, state)?, None)
                    };
                    role_map.push(StructureRoleMapping {
                        from: from.clone(),
                        to: StructureRoleTarget { name, namespace },
                    });
                }
            }
            role_map.sort_by(|a, b| a.from.cmp(&b.from));
            if let Some(schema) = dict.get("Schema") {
                self.raw_value(doc, schema, state)?;
            }
            self.namespace_ids.insert(id, self.namespaces.len());
            self.namespaces.push(StructureNamespace {
                handle: doc.object_handle(id)?,
                uri,
                listed,
                schema: dict.get("Schema").cloned(),
                role_map,
            });
        }
        // Validate all declared mappings, including currently unused entries.
        for namespace in &self.namespaces {
            for mapping in &namespace.role_map {
                self.resolve_role(doc, &mapping.from, Some(&namespace.handle), state)?;
            }
        }
        Ok(())
    }
    pub fn element_namespace<R: Read + Seek>(
        &mut self,
        doc: &PdfDocument<R>,
        node: &PdfDict,
        state: &mut ReadState,
    ) -> PdfResult<Option<ObjectHandle>> {
        let Some(value) = node.get("NS") else {
            return Ok(None);
        };
        self.require_pdf2()?;
        let id = indirect(value)?;
        self.load_namespace(doc, id, false, state)?;
        if !self.namespaces[self.namespace_ids[&id]].listed {
            return Err(invalid("element NS is absent from root Namespaces"));
        }
        Ok(Some(doc.object_handle(id)?))
    }
    pub fn resolve_role<R: Read + Seek>(
        &self,
        doc: &PdfDocument<R>,
        role: &str,
        namespace: Option<&ObjectHandle>,
        state: &mut ReadState,
    ) -> PdfResult<StructureRoleResolution> {
        let mut name = role.to_string();
        let mut target = namespace.cloned();
        if target.is_none() {
            let mut seen = HashSet::new();
            while let Some(next) = state.roles.get(&name).cloned() {
                self.entries(doc, state, 1)?;
                if !seen.insert(name.clone()) {
                    return Ok(StructureRoleResolution {
                        name,
                        namespace: None,
                        namespace_uri: DEFAULT_NAMESPACE.into(),
                        status: StructureRoleStatus::Cycle,
                    });
                }
                if seen.len() > doc.ctx().limits().max_depth {
                    return Err(PdfError::ResourceLimit("structure RoleMap depth".into()));
                }
                name = next;
            }
            return Ok(StructureRoleResolution {
                name,
                namespace: None,
                namespace_uri: DEFAULT_NAMESPACE.into(),
                status: StructureRoleStatus::Endpoint,
            });
        }
        let mut seen = HashSet::new();
        loop {
            self.entries(doc, state, 1)?;
            let Some(handle) = target.as_ref() else {
                return Ok(StructureRoleResolution {
                    name,
                    namespace: None,
                    namespace_uri: DEFAULT_NAMESPACE.into(),
                    status: StructureRoleStatus::Endpoint,
                });
            };
            let ns = &self.namespaces[*self
                .namespace_ids
                .get(&handle.object_id())
                .ok_or_else(|| invalid("namespace mapping target missing"))?];
            // Duplicate URI dictionaries are rejected on load, so object IDs
            // are sufficient here and avoid rehashing long URIs at every hop.
            if !seen.insert(handle.object_id()) {
                return Err(invalid(
                    "namespace role mapping cycle or same-namespace re-entry",
                ));
            }
            if seen.len() > doc.ctx().limits().max_depth {
                return Err(PdfError::ResourceLimit(
                    "structure namespace role depth".into(),
                ));
            }
            let Some(mapping) = ns
                .role_map
                .binary_search_by(|m| m.from.cmp(&name))
                .ok()
                .map(|index| &ns.role_map[index])
            else {
                return Ok(StructureRoleResolution {
                    name,
                    namespace: target,
                    namespace_uri: ns.uri.clone(),
                    status: StructureRoleStatus::Endpoint,
                });
            };
            name = mapping.to.name.clone();
            target = mapping.to.namespace.clone();
            if target.is_none() && ns.uri == DEFAULT_NAMESPACE {
                return Err(invalid("role mapping stays in the same namespace"));
            }
        }
    }
    fn attribute<R: Read + Seek>(
        &mut self,
        doc: &PdfDocument<R>,
        raw: &PdfObject,
        state: &mut ReadState,
    ) -> PdfResult<StructureAttribute> {
        self.entries(doc, state, 1)?;
        let value = doc.ctx().resolve(raw)?;
        let (dict, stream) = match &value {
            PdfObject::PdfDict(dict) => (dict, None),
            PdfObject::PdfStream(stream) => {
                (stream.dict(), Some(doc.object_handle(indirect(raw)?)?))
            }
            _ => {
                return Err(invalid(
                    "attribute object must be dictionary or indirect stream",
                ));
            }
        };
        let owner = doc
            .structure_name(dict, "O")?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("attribute O owner required"))?;
        self.token(doc, state, &owner)?;
        let namespace = dict.get("NS").map(indirect).transpose()?;
        let owner = if owner == "NSO" {
            self.require_pdf2()?;
            let id = namespace.ok_or_else(|| invalid("NSO attribute requires NS"))?;
            self.load_namespace(doc, id, false, state)?;
            let uri = &self.namespaces[self.namespace_ids[&id]].uri;
            doc.structure_charge(state, uri.len())?;
            StructureAttributeOwner::Namespace(uri.clone())
        } else {
            if namespace.is_some() {
                return Err(invalid("attribute NS requires O NSO"));
            }
            StructureAttributeOwner::Name(owner)
        };
        let mut values = PdfDict::default();
        for (key, value) in dict {
            if matches!(key.as_str(), "O" | "NS")
                || stream.is_some()
                    && matches!(
                        key.as_str(),
                        "Length"
                            | "Filter"
                            | "DecodeParms"
                            | "DL"
                            | "F"
                            | "FFilter"
                            | "FDecodeParms"
                    )
            {
                continue;
            }
            self.token(doc, state, key)?;
            self.raw_value(doc, value, state)?;
            values.try_insert(key.clone(), value.clone())?;
        }
        Ok(StructureAttribute {
            owner,
            namespace: namespace.map(|id| doc.object_handle(id)).transpose()?,
            values,
            stream,
        })
    }
    fn attribute_objects<R: Read + Seek>(
        &mut self,
        doc: &PdfDocument<R>,
        raw: &PdfObject,
        state: &mut ReadState,
        revisions: bool,
    ) -> PdfResult<Vec<(StructureAttribute, u32)>> {
        let value = doc.ctx().resolve(raw)?;
        let Some(array) = value.as_array() else {
            return Ok(vec![(self.attribute(doc, raw, state)?, 0)]);
        };
        if revisions && array.len() == 0 {
            return Err(invalid("attribute array must not be empty"));
        }
        self.entries(doc, state, array.len())?;
        let mut attributes = Vec::new();
        let mut index = 0;
        while index < array.len() {
            let attribute = self.attribute(doc, array.get(index).unwrap(), state)?;
            index += 1;
            let revision = if revisions
                && index < array.len()
                && doc
                    .ctx()
                    .resolve(array.get(index).unwrap())?
                    .as_number()
                    .is_some()
            {
                let n = self.integer(doc, array.get(index).unwrap())?;
                index += 1;
                n
            } else {
                0
            };
            attributes.push((attribute, revision));
        }
        Ok(attributes)
    }
    pub fn element_attributes<R: Read + Seek>(
        &mut self,
        doc: &PdfDocument<R>,
        node: &PdfDict,
        state: &mut ReadState,
    ) -> PdfResult<(u32, Vec<StructureClassUse>, Vec<StructureAttributeBinding>)> {
        let revision = node
            .get("R")
            .map(|v| self.integer(doc, v))
            .transpose()?
            .unwrap_or(0);
        let mut classes = Vec::new();
        let mut bindings = Vec::new();
        if let Some(raw) = node.get("C") {
            let value = doc.ctx().resolve(raw)?;
            self.entries(doc, state, value.as_array().map_or(1, |array| array.len()))?;
            let items: Vec<_> = if let Some(array) = value.as_array() {
                array.into_iter().collect()
            } else {
                vec![&value]
            };
            let mut index = 0;
            while index < items.len() {
                let name = self.name(doc, items[index], state)?;
                index += 1;
                let rev = if index < items.len()
                    && doc.ctx().resolve(items[index])?.as_number().is_some()
                {
                    let n = self.integer(doc, items[index])?;
                    index += 1;
                    n
                } else {
                    0
                };
                if rev > revision {
                    return Err(invalid("class revision exceeds element R"));
                }
                let class = self.classes.get(&name);
                classes.push(StructureClassUse {
                    name: name.clone(),
                    revision: rev,
                    defined: class.is_some(),
                });
                if let Some(class) = class {
                    for attribute in &class.attributes {
                        self.entries(doc, state, 1)?;
                        let owner = match &attribute.owner {
                            StructureAttributeOwner::Name(name)
                            | StructureAttributeOwner::Namespace(name) => name,
                        };
                        doc.structure_charge(state, owner.len().saturating_add(name.len()))?;
                        for (key, value) in &attribute.values {
                            self.token(doc, state, key)?;
                            self.raw_value(doc, value, state)?;
                        }
                        bindings.push(StructureAttributeBinding {
                            attribute: attribute.clone(),
                            revision: rev,
                            source: StructureAttributeSource::Class(name.clone()),
                        });
                    }
                }
            }
        }
        if let Some(raw) = node.get("A") {
            for (attribute, rev) in self.attribute_objects(doc, raw, state, true)? {
                if rev > revision {
                    return Err(invalid("attribute revision exceeds element R"));
                }
                bindings.push(StructureAttributeBinding {
                    attribute,
                    revision: rev,
                    source: StructureAttributeSource::Direct,
                });
            }
        }
        Ok((revision, classes, bindings))
    }
}
