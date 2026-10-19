//! Selected-page projection and ownership rebinding, inside page transactions.
use super::*;
use crate::document::resource_import::Importer;
use crate::document::structure::{StructureClassUse, StructureElement, StructureRoleStatus};

type SemanticState = (
    String,
    String,
    StructureRoleStatus,
    Vec<StructureClassUse>,
    Option<bool>,
);
fn semantic_state(element: &StructureElement) -> SemanticState {
    (
        element.resolved_role.namespace_uri.clone(),
        element.resolved_role.name.clone(),
        element.resolved_role.status,
        element.classes.clone(),
        (element.attribute_bindings.len() > 1).then_some(element.attributes_use_array_order),
    )
}

pub(crate) struct StructureTransferPlan {
    source: Option<Graph>,
    target: Option<Graph>,
    namespaces: Vec<ObjectId>,
    same_document: bool,
}
pub(crate) struct StructureDeletionPlan(Option<Graph>);
impl StructureTransferPlan {
    pub(in crate::document) fn prepare<S: Read + Seek, T: Read + Seek>(
        &self,
        importer: &mut Importer<'_, S, T>,
    ) {
        if let Some(graph) = &self.source {
            importer.blocked.insert(graph.root);
            importer.blocked.extend(graph.nodes.keys().copied());
            if self.same_document {
                importer.isolate_structure = true;
                // Namespace definitions are immutable in this workflow. Reuse
                // them rather than creating same-URI dictionary aliases.
                for id in &self.namespaces {
                    importer.mapped.insert(*id, *id);
                }
            }
        }
    }
}
fn association_page(a: &StructureAssociation) -> ObjectId {
    match a {
        StructureAssociation::MarkedContent { page, .. }
        | StructureAssociation::ObjectReference { page, .. } => page.object_id(),
    }
}

fn portable_data(value: &PdfObject, maximum: usize, depth_limit: usize) -> PdfResult<bool> {
    let mut pending = vec![(value, 0usize)];
    let mut work = 0usize;
    while let Some((value, depth)) = pending.pop() {
        work = work.saturating_add(1);
        if work > maximum || depth > depth_limit || pending.len() > maximum.saturating_sub(work) {
            return Err(PdfError::ResourceLimit("structure map comparison".into()));
        }
        match value {
            PdfObject::PdfReference(_) | PdfObject::PdfStream(_) => return Ok(false),
            PdfObject::PdfArray(a) => pending.extend(a.into_iter().map(|v| (v, depth + 1))),
            PdfObject::PdfDict(d) => pending.extend(d.into_iter().map(|(_, v)| (v, depth + 1))),
            _ => {}
        }
    }
    Ok(true)
}

fn rebind_forms<R: Read + Seek>(
    doc: &PdfDocument<R>,
    graph: &mut Graph,
    selected: &HashSet<ObjectId>,
) -> PdfResult<()> {
    let pages = doc.structure_form_pages()?;
    for node in graph.nodes.values_mut() {
        for kid in &mut node.kids {
            if let Kid::Content {
                raw,
                association:
                    StructureAssociation::MarkedContent {
                        page,
                        domain: StructureContentDomain::Form(form),
                        ..
                    },
            } = kid
            {
                if selected.contains(&page.object_id()) {
                    continue;
                }
                let candidates: Vec<_> = pages
                    .get(&form.object_id())
                    .into_iter()
                    .flatten()
                    .filter(|id| selected.contains(id))
                    .copied()
                    .collect();
                if candidates.len() > 1 {
                    return Err(PdfError::UnsupportedFeature(
                        "shared Form declaration has multiple surviving page candidates".into(),
                    ));
                }
                if let Some(id) = candidates.first() {
                    *page = doc.object_handle(*id)?;
                    let PdfObject::PdfDict(raw) = raw else {
                        return Err(invalid("normalized MCR dictionary"));
                    };
                    raw.try_insert("Pg".into(), reference(*id))?;
                }
            }
        }
    }
    Ok(())
}

fn project<R: Read + Seek>(
    doc: &PdfDocument<R>,
    graph: &mut Graph,
    selected: &HashSet<ObjectId>,
    all: bool,
    deleting: bool,
) -> PdfResult<()> {
    let mut order = Vec::new();
    let mut pending: Vec<_> = graph
        .kids
        .iter()
        .filter_map(|k| {
            if let Kid::Element(id) = k {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    while let Some(id) = pending.pop() {
        order.push(id);
        pending.extend(graph.node(id)?.kids.iter().filter_map(|k| {
            if let Kid::Element(id) = k {
                Some(*id)
            } else {
                None
            }
        }));
    }
    let mut flags: HashMap<ObjectId, (bool, bool)> = HashMap::new();
    for id in order.iter().rev() {
        let node = graph.node(*id)?;
        let mut state = (false, false);
        if let Some(pg) = node.dict.get("Pg") {
            if selected.contains(&indirect(pg)?) {
                state.0 = true;
            } else {
                state.1 = true;
            }
        }
        for kid in &node.kids {
            let (yes, no) = match kid {
                Kid::Element(id) => flags[id],
                Kid::Content { association, .. } => {
                    let yes = selected.contains(&association_page(association));
                    (yes, !yes)
                }
            };
            state.0 |= yes;
            state.1 |= no;
        }
        if node.dict.contains_key("ActualText")
            && !all
            && (state == (true, true) || state == (false, false))
        {
            return Err(PdfError::UnsupportedFeature(
                "page projection cannot split or assign an unbound element ActualText".into(),
            ));
        }
        // All-page imports retain metadata-only nodes. Deletion also retains
        // pre-existing empty, page-unbound nodes, not emptied content branches.
        if all || deleting && node.kids.is_empty() && node.dict.get("Pg").is_none() {
            state.0 = true;
        }
        flags.insert(*id, state);
    }
    graph
        .kids
        .retain(|k| matches!(k,Kid::Element(id) if flags[id].0));
    graph.nodes.retain(|id, _| flags[id].0);
    for node in graph.nodes.values_mut() {
        node.kids.retain(|k| match k {
            Kid::Element(id) => flags[id].0,
            Kid::Content { association, .. } => selected.contains(&association_page(association)),
        });
        if node
            .dict
            .get("Pg")
            .map(indirect)
            .transpose()?
            .is_some_and(|pg| !selected.contains(&pg))
        {
            node.dict.remove("Pg");
        }
    }
    if order.len() > doc.ctx().limits().max_objects {
        return Err(PdfError::ResourceLimit("structure projection nodes".into()));
    }
    Ok(())
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Called only by relationship workflows. Broader read support does not
    /// authorize rewriting IDTree/Ref/unknown extension relationships.
    pub(crate) fn validate_structure_relationships(&self) -> PdfResult<bool> {
        let graph = self.editable_structure()?;
        if graph.is_none() && self.structure_has_content_ownership()? {
            return Err(invalid(
                "content ownership without a supported structure tree",
            ));
        }
        Ok(graph.is_some())
    }

    pub(crate) fn plan_structure_transfer<S: Read + Seek>(
        &self,
        source: &PdfDocument<S>,
        selected: &HashSet<ObjectId>,
    ) -> PdfResult<StructureTransferPlan> {
        let mut from = source.editable_structure()?;
        let target = self.editable_structure()?;
        let same_document = self.identity() == source.identity();
        let namespaces = source
            .structure_tree()?
            .map(|t| {
                t.namespaces
                    .into_iter()
                    .map(|n| n.handle.object_id())
                    .collect()
            })
            .unwrap_or_default();
        if let Some(graph) = &mut from {
            self.require_structure_edit()?;
            let all = selected.len() == source.try_page_count()?;
            rebind_forms(source, graph, selected)?;
            project(source, graph, selected, all, false)?;
            if !same_document {
                let source_tree = source.structure_tree()?.unwrap();
                if let Some(tree) = self.structure_tree()?
                    && source_tree
                        .namespaces
                        .iter()
                        .any(|n| tree.namespaces.iter().any(|other| other.uri == n.uri))
                {
                    return Err(PdfError::UnsupportedFeature("import namespace URI conflict; explicit definition reconciliation required".into()));
                }
            }
        }
        Ok(StructureTransferPlan {
            source: from,
            target,
            namespaces,
            same_document,
        })
    }

    pub(in crate::document) fn apply_structure_transfer<S: Read + Seek>(
        &self,
        importer: &mut Importer<'_, S, R>,
        plan: StructureTransferPlan,
    ) -> PdfResult<()> {
        let Some(mut source) = plan.source else {
            self.structure_associations()?;
            return Ok(());
        };
        let previous_tree = self.structure_tree()?;
        let source_tree = importer
            .source
            .structure_tree()?
            .ok_or_else(|| invalid("source tree disappeared during transfer"))?;
        let mut target = if let Some(target) = plan.target {
            target
        } else {
            let root = self.try_insert_object(PdfObject::PdfDict(PdfDict::default()))?;
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
        };
        importer.mapped.insert(source.root, target.root);
        for id in source.nodes.keys() {
            let new = self.try_insert_object(PdfObject::PdfNull)?;
            importer.mapped.insert(*id, new);
        }
        let mut semantics: HashMap<ObjectId, SemanticState> = previous_tree
            .into_iter()
            .flat_map(|tree| tree.elements)
            .map(|e| (e.handle.object_id(), semantic_state(&e)))
            .collect();
        for element in source_tree.elements {
            if source.nodes.contains_key(&element.handle.object_id()) {
                semantics.insert(
                    importer.mapped[&element.handle.object_id()],
                    semantic_state(&element),
                );
            }
        }
        if !plan.same_document {
            for key in ["RoleMap", "ClassMap"] {
                if let Some(raw) = source.dict.get(key) {
                    let dict = importer
                        .source
                        .ctx()
                        .resolve(raw)?
                        .into_dict()
                        .ok_or_else(|| invalid("structure map dictionary"))?;
                    let mut existing = target
                        .dict
                        .get(key)
                        .map(|v| {
                            self.ctx()
                                .resolve(v)?
                                .into_dict()
                                .ok_or_else(|| invalid("target map dictionary"))
                        })
                        .transpose()?
                        .unwrap_or_default();
                    for (name, value) in &dict {
                        if let Some(old) = existing.get(name) {
                            let old = self.ctx().resolve(old)?;
                            let incoming = importer.source.ctx().resolve(value)?;
                            // Equal numeric reference IDs in distinct documents
                            // are not equal definitions. Never alias opaque graphs.
                            if old != incoming
                                || !portable_data(
                                    &old,
                                    self.ctx().limits().max_objects,
                                    self.ctx().limits().max_depth,
                                )?
                                || !portable_data(
                                    &incoming,
                                    importer.source.ctx().limits().max_objects,
                                    importer.source.ctx().limits().max_depth,
                                )?
                            {
                                return Err(PdfError::UnsupportedFeature(format!(
                                    "structure {key} name conflict: {name}"
                                )));
                            }
                        } else {
                            existing
                                .try_insert(name.clone(), importer.object(value.clone(), 0)?)?;
                        }
                    }
                    target
                        .dict
                        .try_insert(key.into(), PdfObject::PdfDict(existing))?;
                }
            }
            if let Some(raw) = source.dict.get("Namespaces") {
                let old = target
                    .dict
                    .get("Namespaces")
                    .map(|v| self.ctx().resolve(v))
                    .transpose()?;
                let mut values = old
                    .as_ref()
                    .and_then(PdfObject::as_array)
                    .cloned()
                    .unwrap_or_default();
                let incoming = importer.source.ctx().resolve(raw)?;
                for value in incoming
                    .as_array()
                    .ok_or_else(|| invalid("Namespaces array"))?
                {
                    values.try_add_obj(importer.object(value.clone(), 0)?)?;
                }
                target
                    .dict
                    .try_insert("Namespaces".into(), PdfObject::PdfArray(values))?;
            }
        }
        let mapped_ids: HashSet<_> = importer
            .mapped
            .iter()
            .filter(|(a, b)| !plan.same_document || a != b)
            .map(|(_, b)| *b)
            .collect();
        let owners = self.structure_live_owners()?;
        let mut next = target
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
        if let Some(value) = target.dict.get("ParentTreeNextKey") {
            next = next.max(
                self.ctx()
                    .resolve(value)?
                    .as_number()
                    .ok_or_else(|| invalid("ParentTreeNextKey number"))?
                    .as_u32_checked()?,
            );
        }
        let mut new_owners: Vec<_> = owners
            .into_iter()
            .filter(|(id, _)| mapped_ids.contains(id))
            .collect();
        new_owners.sort_by_key(|(id, _)| (id.obj_num(), id.gen_num()));
        let mut slots = 0usize;
        for (id, (old_key, domain)) in new_owners {
            let key = next;
            next = next
                .checked_add(1)
                .ok_or_else(|| invalid("ParentTree key overflow"))?;
            let reverse = if domain {
                let old = importer.source.ctx().resolve(
                    source
                        .reverse
                        .get(&old_key)
                        .ok_or_else(|| invalid("source ParentTree owner key"))?,
                )?;
                let length = old
                    .as_array()
                    .ok_or_else(|| invalid("source MCID array"))?
                    .len();
                slots = slots.saturating_add(length);
                if slots > self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("imported ParentTree slots".into()));
                }
                array((0..length).map(|_| PdfObject::PdfNull))?
            } else {
                PdfObject::PdfNull
            };
            target.reverse.insert(key, reverse);
            let value = self.resolve_object(id)?;
            let updated = match value {
                PdfObject::PdfDict(mut dict) => {
                    dict.try_insert(
                        if domain {
                            "StructParents"
                        } else {
                            "StructParent"
                        }
                        .into(),
                        number(key)?,
                    )?;
                    PdfObject::PdfDict(dict)
                }
                PdfObject::PdfStream(stream) => {
                    let (mut dict, data) = stream.into_raw_parts();
                    dict.try_insert(
                        if domain {
                            "StructParents"
                        } else {
                            "StructParent"
                        }
                        .into(),
                        number(key)?,
                    )?;
                    PdfObject::PdfStream(PdfStream::new(dict, data))
                }
                _ => return Err(invalid("structure owner dictionary")),
            };
            self.update_object(id, updated)?;
        }
        let mapped = importer.mapped.clone();
        let map = |id: ObjectId| {
            mapped
                .get(&id)
                .copied()
                .ok_or_else(|| invalid("structure reference outside selected imported graph"))
        };
        let roots: Vec<_> = source
            .kids
            .iter()
            .map(|k| {
                let Kid::Element(id) = k else { unreachable!() };
                map(*id)
            })
            .collect::<PdfResult<_>>()?;
        // Map raw object data only after every structural ID has been reserved.
        for (id, mut node) in source.nodes.drain() {
            let new = map(id)?;
            let parent = map(node.parent)?;
            node.dict.remove("P");
            node.dict.remove("K");
            let dict = importer
                .object(PdfObject::PdfDict(node.dict), 0)?
                .into_dict()
                .unwrap();
            let mut kids = Vec::new();
            for kid in node.kids {
                kids.push(match kid {
                    Kid::Element(id) => Kid::Element(importer.mapped[&id]),
                    Kid::Content { raw, association } => {
                        let raw = importer.object(raw, 0)?;
                        let association = match association {
                            StructureAssociation::MarkedContent {
                                page, domain, mcid, ..
                            } => {
                                let page =
                                    self.object_handle(importer.mapped[&page.object_id()])?;
                                let domain = match domain {
                                    StructureContentDomain::Page(h) => {
                                        StructureContentDomain::Page(
                                            self.object_handle(importer.mapped[&h.object_id()])?,
                                        )
                                    }
                                    StructureContentDomain::Form(h) => {
                                        StructureContentDomain::Form(self.object_handle(
                                            *importer.mapped.get(&h.object_id()).ok_or_else(
                                                || invalid("unselected Form declaration"),
                                            )?,
                                        )?)
                                    }
                                };
                                StructureAssociation::MarkedContent {
                                    element: self.object_handle(new)?,
                                    page,
                                    domain,
                                    mcid,
                                }
                            }
                            StructureAssociation::ObjectReference { page, object, .. } => {
                                StructureAssociation::ObjectReference {
                                    element: self.object_handle(new)?,
                                    page: self.object_handle(importer.mapped[&page.object_id()])?,
                                    object: self.object_handle(
                                        *importer
                                            .mapped
                                            .get(&object.object_id())
                                            .ok_or_else(|| invalid("unselected OBJR"))?,
                                    )?,
                                }
                            }
                        };
                        Kid::Content { raw, association }
                    }
                });
            }
            target.nodes.insert(new, Node { dict, parent, kids });
        }
        target.kids.extend(roots.into_iter().map(Kid::Element));
        target
            .dict
            .try_insert("ParentTreeNextKey".into(), number(next)?)?;
        self.publish_structure(target)?;
        let actual = self
            .structure_tree()?
            .ok_or_else(|| invalid("transferred tree disappeared"))?;
        if actual.elements.len() != semantics.len()
            || actual
                .elements
                .iter()
                .any(|e| semantics.get(&e.handle.object_id()) != Some(&semantic_state(e)))
        {
            return Err(PdfError::UnsupportedFeature("merged definitions or version would change existing/imported role or class semantics".into()));
        }
        Ok(())
    }

    pub(crate) fn plan_structure_page_deletion(
        &self,
        removed: ObjectId,
    ) -> PdfResult<StructureDeletionPlan> {
        let Some(mut graph) = self.editable_structure()? else {
            return Ok(StructureDeletionPlan(None));
        };
        let selected: HashSet<_> = self
            .collect_page_entries()?
            .into_iter()
            .map(|p| p.page_id)
            .filter(|id| *id != removed)
            .collect();
        rebind_forms(self, &mut graph, &selected)?;
        project(self, &mut graph, &selected, false, true)?;
        Ok(StructureDeletionPlan(Some(graph)))
    }

    pub(crate) fn apply_structure_page_deletion(
        &self,
        plan: StructureDeletionPlan,
    ) -> PdfResult<()> {
        let Some(mut graph) = plan.0 else {
            return Ok(());
        };
        let owners = self.structure_live_owners()?;
        let live: HashSet<_> = owners.into_iter().map(|(_, (key, _))| key).collect();
        graph.reverse.retain(|key, _| live.contains(key));
        self.publish_structure(graph)
    }
}
