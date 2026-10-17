//! Bounded, read-only validation of the supported page/Form ownership model.
use super::{StructureKid, indirect, invalid};
use crate::{
    document::{
        PdfDocument,
        identity::{DocumentStamp, ObjectHandle},
        page_tree::PageTree,
        trees::{TreeKey, TreeKind},
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
    page::content_parser::ContentParser,
    parser::syntax::SyntaxParser,
};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Seek},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureContentDomain {
    Page(ObjectHandle),
    Form(ObjectHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureAssociation {
    MarkedContent {
        element: ObjectHandle,
        page: ObjectHandle,
        domain: StructureContentDomain,
        mcid: u32,
    },
    ObjectReference {
        element: ObjectHandle,
        page: ObjectHandle,
        object: ObjectHandle,
    },
}

/// Declared /K order, validated against reachable content and /ParentTree.
/// This is not a PDF/UA validator or an extraction/visibility result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureAssociations {
    pub stamp: DocumentStamp,
    pub root: ObjectHandle,
    pub items: Vec<StructureAssociation>,
}

#[derive(Default)]
struct Index {
    // Page IDs and Form IDs occupy distinct domains (a live page cannot be a stream).
    mcids: HashMap<ObjectId, HashSet<u32>>,
    domains: HashMap<ObjectId, u32>,
    objects: HashMap<ObjectId, u32>,
    reachable: HashSet<(ObjectId, ObjectId)>,
    annotations: HashSet<(ObjectId, ObjectId)>,
    bytes: usize,
    operations: usize,
    entries: usize,
}

enum Task {
    Form { id: ObjectId, depth: usize },
    Exit(ObjectId),
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Validate page/Form MCIDs, MCR, annotation/XObject OBJR and the
    /// bidirectional ParentTree. Scans operators without interpreting graphics,
    /// ActualText or optional content. Uses the hierarchy's qualified namespace
    /// and local attribute model. Appearance-stream MCR/StmOwn and full Tagged
    /// conformance (including PDF 2.0 content restrictions) remain unsupported.
    ///
    /// Work is cumulatively bounded by decoded bytes, content operations and
    /// object entries, including repeated shared-Form invocations. These are
    /// logical limits, not a complete process-memory budget.
    pub fn structure_associations(&self) -> PdfResult<Option<StructureAssociations>> {
        let Some(tree) = self.structure_tree()? else {
            return Ok(None);
        };
        let root = self.structure_dict(tree.root.object_id())?;
        let mut index = self.structure_content_index()?;
        let page_ids: HashSet<_> = self
            .collect_page_entries()?
            .into_iter()
            .map(|p| p.page_id)
            .collect();
        let mut expected_mcids = HashMap::new();
        let mut expected_objects = HashMap::new();
        let mut object_occurrences = HashSet::new();
        let mut items = Vec::new();
        let elements: HashMap<_, _> = tree
            .elements
            .iter()
            .map(|e| (e.handle.object_id(), e))
            .collect();
        let mut pending: Vec<_> = tree
            .children
            .iter()
            .rev()
            .map(|kid| (tree.root.object_id(), kid))
            .collect();
        while let Some((owner, kid)) = pending.pop() {
            let raw = match kid {
                StructureKid::Element(handle) => {
                    let element = elements[&handle.object_id()];
                    pending.extend(
                        element
                            .children
                            .iter()
                            .rev()
                            .map(|kid| (handle.object_id(), kid)),
                    );
                    continue;
                }
                StructureKid::UnvalidatedContent(raw) => raw,
            };
            let element = elements[&owner];
            self.association_charge(
                &mut index.entries,
                1,
                self.ctx().limits().max_objects,
                "entries",
            )?;
            let value = self.ctx().resolve(raw)?;
            let dict = value.as_dict();
            let page = dict
                .and_then(|d| d.get("Pg"))
                .map(indirect)
                .transpose()?
                .or_else(|| element.page.as_ref().map(ObjectHandle::object_id))
                .ok_or_else(|| invalid("content item requires its own or element Pg"))?;
            if !page_ids.contains(&page) {
                return Err(invalid("content Pg is not a document page"));
            }
            if let Some(dict) = dict {
                if self.structure_name(dict, "Type")?.as_deref() == Some("OBJR") {
                    let object =
                        indirect(dict.get("Obj").ok_or_else(|| invalid("OBJR Obj missing"))?)?;
                    if !index.annotations.contains(&(page, object))
                        && !index.reachable.contains(&(page, object))
                    {
                        return Err(invalid(
                            "OBJR object is not an annotation or invoked XObject on Pg",
                        ));
                    }
                    if !index.objects.contains_key(&object) {
                        return Err(invalid("OBJR object StructParent missing"));
                    }
                    if !object_occurrences.insert((page, object)) {
                        return Err(invalid("duplicate OBJR on page"));
                    }
                    if expected_objects
                        .insert(object, element.handle.object_id())
                        .is_some_and(|old| old != element.handle.object_id())
                    {
                        return Err(invalid("object has multiple structure parents"));
                    }
                    items.push(StructureAssociation::ObjectReference {
                        element: element.handle.clone(),
                        page: self.object_handle(page)?,
                        object: self.object_handle(object)?,
                    });
                    continue;
                }
            }
            let (domain, mcid) = if let Some(dict) = dict {
                if dict.contains_key("StmOwn") {
                    return Err(PdfError::UnsupportedFeature(
                        "structure MCR StmOwn/appearance streams".into(),
                    ));
                }
                let mcid = self.association_integer(
                    dict.get("MCID")
                        .ok_or_else(|| invalid("MCR MCID missing"))?,
                )?;
                let domain = dict.get("Stm").map(indirect).transpose()?.unwrap_or(page);
                if domain != page && !index.reachable.contains(&(page, domain)) {
                    return Err(PdfError::UnsupportedFeature(
                        "MCR Stm must be an invoked Form on Pg; appearance streams are unsupported"
                            .into(),
                    ));
                }
                (domain, mcid)
            } else {
                (page, self.association_integer(&value)?)
            };
            if !index
                .mcids
                .get(&domain)
                .is_some_and(|ids| ids.contains(&mcid))
            {
                return Err(invalid("declared MCID missing in its page/Form domain"));
            }
            if !index.domains.contains_key(&domain) {
                return Err(invalid("MCID domain StructParents missing"));
            }
            if expected_mcids
                .insert((domain, mcid), element.handle.object_id())
                .is_some()
            {
                return Err(invalid("duplicate declared MCID ownership"));
            }
            let domain = if domain == page {
                StructureContentDomain::Page(self.object_handle(domain)?)
            } else {
                StructureContentDomain::Form(self.object_handle(domain)?)
            };
            items.push(StructureAssociation::MarkedContent {
                element: element.handle.clone(),
                page: self.object_handle(page)?,
                domain,
                mcid,
            });
        }
        // Every discovered structural MCID/StructParent needs a forward owner.
        for (domain, ids) in &index.mcids {
            for mcid in ids {
                if !expected_mcids.contains_key(&(*domain, *mcid)) {
                    return Err(invalid("content MCID has no structure K owner"));
                }
            }
        }
        for id in index.objects.keys() {
            if !expected_objects.contains_key(id) {
                return Err(invalid("StructParent object has no OBJR owner"));
            }
        }
        for (page, id) in index.reachable.iter().chain(index.annotations.iter()) {
            if index.objects.contains_key(id) && !object_occurrences.contains(&(*page, *id)) {
                return Err(invalid(
                    "StructParent object requires an OBJR for each rendering page",
                ));
            }
        }
        let reverse = tree
            .unvalidated_parent_tree
            .as_ref()
            .map(|v| self.read_tree(v, TreeKind::Number))
            .transpose()?
            .unwrap_or_default();
        let mut keys = HashMap::new();
        for (id, key) in &index.domains {
            if keys.insert(*key, (*id, true)).is_some() {
                return Err(invalid("duplicate StructParent(s) key"));
            }
        }
        for (id, key) in &index.objects {
            if keys.insert(*key, (*id, false)).is_some() {
                return Err(invalid("duplicate StructParent(s) key"));
            }
        }
        let mut checked = HashSet::new();
        let mut greatest_key = None;
        for (key, value) in reverse {
            let TreeKey::Number(key) = key else {
                unreachable!()
            };
            greatest_key = Some(key);
            let (id, domain) = keys
                .get(&key)
                .copied()
                .ok_or_else(|| PdfError::UnsupportedFeature(
                    "ParentTree key has no reachable owner in the supported page/Form/annotation subset".into()
                ))?;
            checked.insert(key);
            if domain {
                let value = self.ctx().resolve(&value)?;
                let slots = value
                    .as_array()
                    .ok_or_else(|| invalid("MCID ParentTree value must be an array"))?;
                self.association_charge(
                    &mut index.entries,
                    slots.len(),
                    self.ctx().limits().max_objects,
                    "entries",
                )?;
                for (mcid, slot) in slots.into_iter().enumerate() {
                    let mcid = u32::try_from(mcid).map_err(|_| invalid("MCID slot overflow"))?;
                    let expected = expected_mcids.get(&(id, mcid));
                    if matches!(self.ctx().resolve(slot)?, PdfObject::PdfNull) {
                        if expected.is_some() {
                            return Err(invalid("ParentTree null slot has a K owner"));
                        }
                    } else if Some(&indirect(slot)?) != expected {
                        return Err(invalid("ParentTree slot/K ownership mismatch"));
                    }
                }
                if index
                    .mcids
                    .get(&id)
                    .is_some_and(|ids| ids.iter().any(|m| *m as usize >= slots.len()))
                {
                    return Err(invalid("ParentTree array too short for MCID"));
                }
            } else if expected_objects.get(&id) != Some(&indirect(&value)?) {
                return Err(invalid("ParentTree object/OBJR ownership mismatch"));
            }
        }
        if checked.len() != keys.len() {
            return Err(invalid("StructParent(s) key missing from ParentTree"));
        }
        if let Some(next) = root.get("ParentTreeNextKey") {
            let next = self.association_integer(next)?;
            if greatest_key.is_some_and(|key| next <= key) {
                return Err(invalid("ParentTreeNextKey must exceed existing keys"));
            }
        }
        Ok(Some(StructureAssociations {
            stamp: tree.stamp,
            root: tree.root,
            items,
        }))
    }

    fn structure_content_index(&self) -> PdfResult<Index> {
        let mut index = Index::default();
        let pages = self.collect_page_entries()?;
        for page in &pages {
            self.association_charge(
                &mut index.entries,
                1,
                self.ctx().limits().max_objects,
                "entries",
            )?;
            self.association_owner(page.page_id, &page.page_dict, &mut index)?;
            let reference = PdfObject::PdfReference(PdfReference::from_id(page.page_id));
            let resources = PageTree::inherited_attribute(self.ctx(), &reference, "Resources")?
                .map(|v| {
                    v.into_dict()
                        .ok_or_else(|| invalid("page Resources dictionary"))
                })
                .transpose()?
                .unwrap_or_default();
            if let Some(annots) = page.page_dict.get("Annots") {
                let annots = self.ctx().resolve(annots)?;
                let annots = annots.as_array().ok_or_else(|| invalid("Annots array"))?;
                for value in annots {
                    self.association_charge(
                        &mut index.entries,
                        1,
                        self.ctx().limits().max_objects,
                        "entries",
                    )?;
                    let id = indirect(value)?;
                    if !index.annotations.insert((page.page_id, id)) {
                        return Err(invalid("duplicate annotation on page"));
                    }
                    let dict = self.structure_dict(id)?;
                    if dict.contains_key("StructParents") && !dict.contains_key("StructParent") {
                        return Err(PdfError::UnsupportedFeature(
                            "annotation StructParents/appearance-stream MCID ownership".into(),
                        ));
                    }
                    if self
                        .structure_name(&dict, "Type")?
                        .as_deref()
                        .is_some_and(|s| s != "Annot")
                    {
                        return Err(invalid("OBJR annotation Type"));
                    }
                    if let Some(p) = dict.get("P") {
                        if indirect(p)? != page.page_id {
                            return Err(invalid("annotation P/page mismatch"));
                        }
                    }
                    self.association_owner(id, &dict, &mut index)?;
                }
            }
            let bytes = self.association_page_bytes(&page.page_dict, &mut index)?;
            let mut tasks = self.association_scan(
                page.page_id,
                page.page_id,
                bytes,
                &resources,
                0,
                &mut index,
            )?;
            let mut active = HashSet::new();
            while let Some(task) = tasks.pop() {
                match task {
                    Task::Exit(id) => {
                        active.remove(&id);
                    }
                    Task::Form { id, depth } => {
                        if depth >= self.ctx().limits().max_depth {
                            return Err(PdfError::ResourceLimit("structure Form depth".into()));
                        }
                        if !active.insert(id) {
                            return Err(invalid("recursive Form invocation"));
                        }
                        let value = self.resolve_object(id)?;
                        let stream = value
                            .as_stream()
                            .ok_or_else(|| invalid("Form must be a stream"))?;
                        let dict = stream.dict();
                        self.association_owner(id, dict, &mut index)?;
                        let form_resources = dict
                            .get("Resources")
                            .map(|v| {
                                self.ctx()
                                    .resolve(v)?
                                    .into_dict()
                                    .ok_or_else(|| invalid("Form Resources dictionary"))
                            })
                            .transpose()?
                            // ISO 32000-1 7.8.3: obsolete omission falls back to
                            // the page, not the immediately invoking Form.
                            .unwrap_or_else(|| resources.clone());
                        let bytes = stream.decode_data(self.ctx())?;
                        self.association_charge(
                            &mut index.bytes,
                            bytes.len(),
                            self.ctx().limits().max_decoded_bytes,
                            "decoded bytes",
                        )?;
                        let children = self.association_scan(
                            page.page_id,
                            id,
                            bytes,
                            &form_resources,
                            depth,
                            &mut index,
                        )?;
                        tasks.push(Task::Exit(id));
                        tasks.extend(children);
                    }
                }
            }
        }
        Ok(index)
    }

    pub(crate) fn structure_live_owners(&self) -> PdfResult<HashMap<ObjectId, (u32, bool)>> {
        let index = self.structure_content_index()?;
        Ok(index
            .domains
            .into_iter()
            .map(|(id, key)| (id, (key, true)))
            .chain(
                index
                    .objects
                    .into_iter()
                    .map(|(id, key)| (id, (key, false))),
            )
            .collect())
    }

    pub(crate) fn structure_has_content_ownership(&self) -> PdfResult<bool> {
        let index = self.structure_content_index()?;
        Ok(!index.domains.is_empty()
            || !index.objects.is_empty()
            || index.mcids.values().any(|ids| !ids.is_empty()))
    }

    pub(crate) fn structure_form_pages(&self) -> PdfResult<HashMap<ObjectId, HashSet<ObjectId>>> {
        let mut pages: HashMap<ObjectId, HashSet<ObjectId>> = HashMap::new();
        for (page, object) in self.structure_content_index()?.reachable {
            pages.entry(object).or_default().insert(page);
        }
        Ok(pages)
    }

    fn association_integer(&self, value: &PdfObject) -> PdfResult<u32> {
        self.ctx()
            .resolve(value)?
            .as_number()
            .ok_or_else(|| invalid("expected nonnegative integer"))?
            .as_u32_checked()
            .map_err(|_| invalid("expected nonnegative integer"))
    }

    fn association_charge(
        &self,
        used: &mut usize,
        amount: usize,
        limit: usize,
        what: &str,
    ) -> PdfResult<()> {
        *used = used
            .checked_add(amount)
            .filter(|v| *v <= limit)
            .ok_or_else(|| PdfError::ResourceLimit(format!("structure association {what}")))?;
        Ok(())
    }

    fn association_owner(&self, id: ObjectId, dict: &PdfDict, index: &mut Index) -> PdfResult<()> {
        if dict.contains_key("StructParent") && dict.contains_key("StructParents") {
            return Err(invalid(
                "object cannot have both StructParent and StructParents",
            ));
        }
        for (name, target) in [
            ("StructParents", &mut index.domains),
            ("StructParent", &mut index.objects),
        ] {
            if let Some(value) = dict.get(name) {
                target.insert(id, self.association_integer(value)?);
            }
        }
        Ok(())
    }

    fn association_page_bytes(&self, page: &PdfDict, index: &mut Index) -> PdfResult<Vec<u8>> {
        let Some(value) = page.get("Contents") else {
            return Ok(Vec::new());
        };
        let value = self.ctx().resolve(value)?;
        let streams = match value {
            PdfObject::PdfNull => return Ok(Vec::new()),
            PdfObject::PdfArray(array) => array.into_iter().cloned().collect::<Vec<_>>(),
            value => vec![value],
        };
        let mut bytes = Vec::new();
        for value in streams {
            self.association_charge(
                &mut index.entries,
                1,
                self.ctx().limits().max_objects,
                "entries",
            )?;
            let value = self.ctx().resolve(&value)?;
            let stream = value
                .as_stream()
                .ok_or_else(|| invalid("page Contents must contain streams"))?;
            let data = stream.decode_data(self.ctx())?;
            self.association_charge(
                &mut index.bytes,
                data.len(),
                self.ctx().limits().max_decoded_bytes,
                "decoded bytes",
            )?;
            bytes.extend_from_slice(&data);
        }
        Ok(bytes)
    }

    fn association_resource(
        &self,
        resources: &PdfDict,
        category: &str,
        name: &str,
    ) -> PdfResult<PdfObject> {
        let group = resources
            .get(category)
            .ok_or_else(|| invalid("content resource category missing"))?;
        let group = self.ctx().resolve(group)?;
        group
            .as_dict()
            .and_then(|d| d.get(name))
            .cloned()
            .ok_or_else(|| invalid("content resource missing"))
    }

    fn association_scan(
        &self,
        page: ObjectId,
        domain: ObjectId,
        bytes: Vec<u8>,
        resources: &PdfDict,
        depth: usize,
        index: &mut Index,
    ) -> PdfResult<Vec<Task>> {
        self.association_scan_inner(page, domain, bytes, resources, depth, index)
            .map_err(|error| match error {
                PdfError::DocumentError(message) => PdfError::DocumentError(format!(
                    "{message} (page {page:?}, content domain {domain:?})"
                )),
                PdfError::ContentParseError(message) => PdfError::ContentParseError(format!(
                    "{message} (page {page:?}, content domain {domain:?})"
                )),
                other => other,
            })
    }

    fn association_scan_inner(
        &self,
        page: ObjectId,
        domain: ObjectId,
        bytes: Vec<u8>,
        resources: &PdfDict,
        depth: usize,
        index: &mut Index,
    ) -> PdfResult<Vec<Task>> {
        let reader = StreamReader::try_new(Cursor::new(bytes))?;
        let mut parser =
            ContentParser::new(SyntaxParser::with_options(reader, self.ctx().options()));
        let mut ids = HashSet::new();
        let mut nesting = 0usize;
        let mut tasks = Vec::new();
        while let Some(op) = parser.read_operator()? {
            self.association_charge(
                &mut index.operations,
                1,
                self.ctx().limits().max_content_operations,
                "content operations",
            )?;
            match op.name() {
                "BMC" | "BDC" => {
                    let bdc = op.name() == "BDC";
                    if op.num_operands() != if bdc { 2 } else { 1 } {
                        return Err(invalid("BMC/BDC operands"));
                    }
                    op.get_as_name(0)?;
                    if bdc {
                        let properties = op.operand(1)?;
                        let properties = if let Some(name) = properties.as_name() {
                            self.association_resource(resources, "Properties", name.name())?
                        } else {
                            properties.clone()
                        };
                        let properties = self.ctx().resolve(&properties)?;
                        let dict = properties
                            .as_dict()
                            .ok_or_else(|| invalid("BDC properties dictionary"))?;
                        if let Some(mcid) = dict.get("MCID") {
                            let mcid = self.association_integer(mcid)?;
                            if !ids.insert(mcid) {
                                return Err(invalid("duplicate MCID in page/Form domain"));
                            }
                            self.association_charge(
                                &mut index.entries,
                                1,
                                self.ctx().limits().max_objects,
                                "entries",
                            )?;
                        }
                    }
                    nesting = nesting
                        .checked_add(1)
                        .filter(|n| *n <= self.ctx().limits().max_depth)
                        .ok_or_else(|| {
                            PdfError::ResourceLimit("structure marked-content depth".into())
                        })?;
                }
                "EMC" => {
                    if op.num_operands() != 0 || nesting == 0 {
                        return Err(invalid("unmatched EMC"));
                    }
                    nesting -= 1;
                }
                "Do" => {
                    if op.num_operands() != 1 {
                        return Err(invalid("Do operands"));
                    }
                    let value =
                        self.association_resource(resources, "XObject", op.get_as_name(0)?.name())?;
                    let id = indirect(&value)?;
                    let value = self.ctx().resolve(&value)?;
                    let stream = value.as_stream().ok_or_else(|| invalid("XObject stream"))?;
                    self.association_charge(
                        &mut index.entries,
                        1,
                        self.ctx().limits().max_objects,
                        "entries",
                    )?;
                    index.reachable.insert((page, id));
                    self.association_owner(id, stream.dict(), index)?;
                    match self.structure_name(stream.dict(), "Subtype")?.as_deref() {
                        Some("Form") => tasks.push(Task::Form {
                            id,
                            depth: depth + 1,
                        }),
                        Some("Image") => {}
                        _ => {
                            return Err(PdfError::UnsupportedFeature(
                                "structure XObject subtype".into(),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        if nesting != 0 {
            return Err(invalid("unterminated marked-content sequence"));
        }
        if let Some(previous) = index.mcids.get(&domain) {
            if previous != &ids {
                return Err(invalid("shared Form has resource-dependent MCIDs"));
            }
        } else {
            index.mcids.insert(domain, ids);
        }
        Ok(tasks)
    }
}
