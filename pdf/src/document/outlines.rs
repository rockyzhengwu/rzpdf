//! Hierarchical outline editing with stable identities and linked ownership.
use super::{PdfDocument, destination::Destination, identity::ObjectHandle};
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
    },
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek},
};

#[derive(Debug, Clone, PartialEq)]
pub enum OutlineTarget {
    Destination(Destination),
    Named(String),
}

/// Preorder projection. A parent address is stable across moves and renames.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineItem {
    pub handle: ObjectHandle,
    pub parent: Option<ObjectHandle>,
    pub title: String,
    pub target: OutlineTarget,
    pub open: bool,
}

struct Node {
    id: ObjectId,
    dict: PdfDict,
    parent: Option<ObjectId>,
    children: Vec<ObjectId>,
    open: bool,
}
struct Graph {
    root: Option<ObjectId>,
    dict: PdfDict,
    children: Vec<ObjectId>,
    nodes: Vec<Node>,
    indices: HashMap<ObjectId, usize>,
}

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn required(value: Option<&PdfObject>) -> PdfResult<ObjectId> {
    value
        .and_then(PdfObject::as_reference)
        .map(PdfReference::id)
        .ok_or_else(|| PdfError::DocumentError("outline ownership must be indirect".into()))
}

impl Graph {
    fn index(&self, id: ObjectId) -> PdfResult<usize> {
        self.indices
            .get(&id)
            .copied()
            .ok_or_else(|| PdfError::DocumentError("outline item missing".into()))
    }
    fn visible(&self, children: &[ObjectId], depth: usize, max_depth: usize) -> PdfResult<usize> {
        if depth >= max_depth.min(128) {
            return Err(PdfError::ResourceLimit("outline depth".into()));
        }
        let mut count = 0usize;
        for id in children {
            let node = &self.nodes[self.index(*id)?];
            count = count
                .checked_add(1)
                .ok_or_else(|| PdfError::ResourceLimit("outline Count".into()))?;
            // Visit closed children as well to validate the entire graph depth.
            let inner = self.visible(&node.children, depth + 1, max_depth)?;
            if node.open {
                count = count
                    .checked_add(inner)
                    .ok_or_else(|| PdfError::ResourceLimit("outline Count".into()))?;
            }
        }
        Ok(count)
    }
    fn children_mut(&mut self, parent: Option<ObjectId>) -> PdfResult<&mut Vec<ObjectId>> {
        if let Some(id) = parent {
            let index = self.index(id)?;
            Ok(&mut self.nodes[index].children)
        } else {
            Ok(&mut self.children)
        }
    }
}

impl<R: Read + Seek> PdfDocument<R> {
    fn outline_target(&self, dict: &PdfDict) -> PdfResult<OutlineTarget> {
        if dict.get("Dest").is_some() && dict.get("A").is_some() {
            return Err(PdfError::DocumentError(
                "outline has both Dest and A".into(),
            ));
        }
        let dest = if let Some(dest) = dict.get("Dest") {
            self.ctx().resolve(dest)?
        } else {
            let action = self
                .ctx()
                .resolve(dict.get("A").ok_or_else(|| {
                    PdfError::UnsupportedFeature("outline without destination".into())
                })?)?
                .into_dict()
                .ok_or_else(|| {
                    PdfError::DocumentError("outline action must be a dictionary".into())
                })?;
            if action
                .get("S")
                .and_then(PdfObject::as_name)
                .map(PdfName::name)
                != Some("GoTo")
            {
                return Err(PdfError::UnsupportedFeature(
                    "only GoTo outline actions are editable".into(),
                ));
            }
            self.ctx().resolve(
                action
                    .get("D")
                    .ok_or_else(|| PdfError::DocumentError("GoTo destination missing".into()))?,
            )?
        };
        let name = if let Some(s) = dest.as_string() {
            Some(s.text_content_with_limits(&self.ctx().limits())?)
        } else {
            dest.as_name().map(|n| n.name().to_owned())
        };
        if let Some(name) = name {
            if !self
                .named_destinations()?
                .iter()
                .any(|(key, _)| key == &name)
            {
                return Err(PdfError::DocumentError(
                    "outline named destination missing".into(),
                ));
            }
            Ok(OutlineTarget::Named(name))
        } else {
            Ok(OutlineTarget::Destination(self.decode_destination(&dest)?))
        }
    }

    fn encode_outline_target(&self, target: &OutlineTarget) -> PdfResult<PdfObject> {
        match target {
            OutlineTarget::Destination(dest) => self.encode_destination(dest),
            OutlineTarget::Named(name) => {
                if !self
                    .named_destinations()?
                    .iter()
                    .any(|(key, _)| key == name)
                {
                    return Err(PdfError::DocumentError(
                        "outline named destination missing".into(),
                    ));
                }
                Ok(PdfObject::PdfString(PdfString::from_text_with_limits(
                    name,
                    &self.ctx().limits(),
                )?))
            }
        }
    }

    fn outline_graph(&self) -> PdfResult<Graph> {
        let root = self.ctx().get_root_owned()?;
        let Some(value) = root.get("Outlines") else {
            return Ok(Graph {
                root: None,
                dict: PdfDict::default(),
                children: vec![],
                nodes: vec![],
                indices: HashMap::new(),
            });
        };
        let root_id = required(Some(value))?;
        let dict = self
            .resolve_object(root_id)?
            .into_dict()
            .ok_or_else(|| PdfError::DocumentError("Outlines must be a dictionary".into()))?;
        let mut graph = Graph {
            root: Some(root_id),
            dict: dict.clone(),
            children: vec![],
            nodes: vec![],
            indices: HashMap::new(),
        };
        let mut seen = HashSet::from([root_id]);
        self.read_outline_level(&dict, root_id, None, 0, &mut graph, &mut seen)?;
        let visible = graph.visible(&graph.children, 0, self.ctx().limits().max_depth)?;
        if let Some(count) = dict.get("Count") {
            let count = self
                .ctx()
                .resolve(count)?
                .as_number()
                .ok_or_else(|| PdfError::DocumentError("outline Count must be integer".into()))?
                .as_u32_checked()?;
            if count as usize != visible {
                return Err(PdfError::DocumentError(
                    "inconsistent outline root Count".into(),
                ));
            }
        }
        for node in &graph.nodes {
            let visible = graph.visible(&node.children, 0, self.ctx().limits().max_depth)?;
            if let Some(count) = node.dict.get("Count") {
                let n = self
                    .ctx()
                    .resolve(count)?
                    .as_number()
                    .ok_or_else(|| PdfError::DocumentError("outline Count must be integer".into()))?
                    .as_i32_checked()?;
                if n.unsigned_abs() as usize != visible {
                    return Err(PdfError::DocumentError(
                        "inconsistent outline item Count".into(),
                    ));
                }
            } else if !node.children.is_empty() {
                return Err(PdfError::DocumentError(
                    "outline parent Count missing".into(),
                ));
            }
        }
        Ok(graph)
    }

    fn read_outline_level(
        &self,
        parent_dict: &PdfDict,
        owner: ObjectId,
        parent: Option<ObjectId>,
        depth: usize,
        graph: &mut Graph,
        seen: &mut HashSet<ObjectId>,
    ) -> PdfResult<()> {
        if depth >= self.ctx().limits().max_depth.min(128) {
            return Err(PdfError::ResourceLimit("outline depth".into()));
        }
        let (first, last) = (parent_dict.get("First"), parent_dict.get("Last"));
        if first.is_none() && last.is_none() {
            return Ok(());
        }
        let last = required(last)?;
        let mut next = Some(required(first)?);
        let mut previous = None;
        while let Some(id) = next {
            if seen.len() >= self.ctx().limits().max_objects || !seen.insert(id) {
                return Err(PdfError::ResourceLimit("outline count or cycle".into()));
            }
            let dict = self.resolve_object(id)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("outline item must be a dictionary".into())
            })?;
            if required(dict.get("Parent"))? != owner
                || dict.get("Prev").map(|v| required(Some(v))).transpose()? != previous
            {
                return Err(PdfError::DocumentError("broken outline Parent/Prev".into()));
            }
            let title =
                self.ctx()
                    .resolve(dict.get("Title").ok_or_else(|| {
                        PdfError::DocumentError("outline Title missing".into())
                    })?)?;
            title
                .as_string()
                .ok_or_else(|| PdfError::DocumentError("outline Title must be string".into()))?
                .text_content_with_limits(&self.ctx().limits())?;
            self.outline_target(&dict)?;
            if dict.get("AA").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "outline additional actions".into(),
                ));
            }
            let count = dict
                .get("Count")
                .map(|n| {
                    self.ctx()
                        .resolve(n)?
                        .as_number()
                        .ok_or_else(|| {
                            PdfError::DocumentError("outline Count must be integer".into())
                        })?
                        .as_i32_checked()
                })
                .transpose()?
                .unwrap_or(0);
            graph.indices.insert(id, graph.nodes.len());
            graph.nodes.push(Node {
                id,
                dict: dict.clone(),
                parent,
                children: vec![],
                open: count >= 0,
            });
            graph.children_mut(parent)?.push(id);
            self.read_outline_level(&dict, id, Some(id), depth + 1, graph, seen)?;
            next = dict.get("Next").map(|v| required(Some(v))).transpose()?;
            previous = Some(id);
        }
        if previous != Some(last) {
            return Err(PdfError::DocumentError("broken outline Last".into()));
        }
        Ok(())
    }

    pub fn outline_items(&self) -> PdfResult<Vec<OutlineItem>> {
        let graph = self.outline_graph()?;
        let mut result = Vec::new();
        for node in graph.nodes {
            result.push(OutlineItem {
                handle: self.object_handle(node.id)?,
                parent: node.parent.map(|p| self.object_handle(p)).transpose()?,
                title: self
                    .ctx()
                    .resolve(node.dict.get("Title").unwrap())?
                    .as_string()
                    .unwrap()
                    .text_content_with_limits(&self.ctx().limits())?,
                target: self.outline_target(&node.dict)?,
                open: node.open,
            });
        }
        Ok(result)
    }

    pub fn add_outline(
        &self,
        parent: Option<&ObjectHandle>,
        title: &str,
        target: &OutlineTarget,
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_assembly_permission()?;
        let parent = parent.map(|p| self.validate_handle(p)).transpose()?;
        let title = PdfString::from_text_with_limits(title, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let mut graph = doc.outline_graph()?;
            if graph.nodes.len() >= doc.ctx().limits().max_objects.saturating_sub(1) {
                return Err(PdfError::ResourceLimit("outline count".into()));
            }
            if let Some(parent) = parent {
                graph.index(parent)?;
            }
            let mut dict = PdfDict::default();
            dict.try_insert("Title".into(), PdfObject::PdfString(title))?;
            dict.try_insert("Dest".into(), doc.encode_outline_target(target)?)?;
            let id = doc.try_insert_object(PdfObject::PdfDict(dict.clone()))?;
            graph.indices.insert(id, graph.nodes.len());
            graph.nodes.push(Node {
                id,
                dict,
                parent,
                children: vec![],
                open: true,
            });
            graph.children_mut(parent)?.push(id);
            doc.write_outline_graph(graph)?;
            doc.object_handle(id)
        })
    }

    pub fn update_outline(
        &self,
        handle: &ObjectHandle,
        title: &str,
        target: &OutlineTarget,
        open: bool,
    ) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        let id = self.validate_handle(handle)?;
        let title = PdfString::from_text_with_limits(title, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let mut graph = doc.outline_graph()?;
            let i = graph.index(id)?;
            graph.nodes[i]
                .dict
                .try_insert("Title".into(), PdfObject::PdfString(title))?;
            graph.nodes[i].dict.remove("A");
            graph.nodes[i]
                .dict
                .try_insert("Dest".into(), doc.encode_outline_target(target)?)?;
            graph.nodes[i].open = open;
            doc.write_outline_graph(graph)
        })
    }

    /// Move a subtree. Index is the insertion position after detaching it from
    /// its old siblings. Moving into its own descendants is rejected.
    pub fn move_outline(
        &self,
        handle: &ObjectHandle,
        parent: Option<&ObjectHandle>,
        index: usize,
    ) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        let id = self.validate_handle(handle)?;
        let parent = parent.map(|p| self.validate_handle(p)).transpose()?;
        self.edit_transaction(|doc| {
            let mut graph = doc.outline_graph()?;
            let i = graph.index(id)?;
            let mut ancestor = parent;
            while let Some(p) = ancestor {
                if p == id {
                    return Err(PdfError::DocumentError(
                        "outline move would create cycle".into(),
                    ));
                }
                ancestor = graph.nodes[graph.index(p)?].parent;
            }
            let old_parent = graph.nodes[i].parent;
            graph.children_mut(old_parent)?.retain(|p| *p != id);
            let siblings = graph.children_mut(parent)?;
            if index > siblings.len() {
                return Err(PdfError::DocumentError("outline insertion index".into()));
            }
            siblings.insert(index, id);
            graph.nodes[i].parent = parent;
            doc.write_outline_graph(graph)
        })
    }

    /// Detach the item and all descendants; retained objects are not erased.
    pub fn remove_outline(&self, handle: &ObjectHandle) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        let id = self.validate_handle(handle)?;
        self.edit_transaction(|doc| {
            let mut graph = doc.outline_graph()?;
            let parent = graph.nodes[graph.index(id)?].parent;
            let mut removed = HashSet::from([id]);
            // Preorder means an ancestor is seen before its descendants.
            for node in &graph.nodes {
                if node.parent.is_some_and(|p| removed.contains(&p)) {
                    removed.insert(node.id);
                }
            }
            graph.children_mut(parent)?.retain(|p| *p != id);
            graph.nodes.retain(|n| !removed.contains(&n.id));
            graph.indices = graph
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (n.id, i))
                .collect();
            doc.write_outline_graph(graph)
        })
    }

    fn write_outline_graph(&self, mut graph: Graph) -> PdfResult<()> {
        let visible = graph.visible(&graph.children, 0, self.ctx().limits().max_depth)?;
        let root_id = match graph.root {
            Some(id) => id,
            None => self.try_insert_object(PdfObject::PdfDict(PdfDict::default()))?,
        };
        let mut neighbors = HashMap::new();
        for children in
            std::iter::once(&graph.children).chain(graph.nodes.iter().map(|n| &n.children))
        {
            for (index, id) in children.iter().enumerate() {
                neighbors.insert(
                    *id,
                    (
                        index.checked_sub(1).map(|i| children[i]),
                        children.get(index + 1).copied(),
                    ),
                );
            }
        }
        for node in &graph.nodes {
            let (previous, next) = neighbors
                .get(&node.id)
                .ok_or_else(|| PdfError::DocumentError("outline sibling missing".into()))?;
            let mut dict = node.dict.clone();
            for key in ["Parent", "Prev", "Next", "First", "Last", "Count"] {
                dict.remove(key);
            }
            dict.try_insert("Parent".into(), reference(node.parent.unwrap_or(root_id)))?;
            if let Some(previous) = previous {
                dict.try_insert("Prev".into(), reference(*previous))?;
            }
            if let Some(next) = next {
                dict.try_insert("Next".into(), reference(*next))?;
            }
            if let (Some(first), Some(last)) = (node.children.first(), node.children.last()) {
                dict.try_insert("First".into(), reference(*first))?;
                dict.try_insert("Last".into(), reference(*last))?;
                let count = i32::try_from(graph.visible(
                    &node.children,
                    0,
                    self.ctx().limits().max_depth,
                )?)
                .map_err(|_| PdfError::ResourceLimit("outline Count".into()))?;
                let count = if node.open { count } else { -count };
                dict.try_insert(
                    "Count".into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(count.to_string().as_bytes())?),
                )?;
            }
            self.update_object(node.id, PdfObject::PdfDict(dict))?;
        }
        for key in ["First", "Last", "Count"] {
            graph.dict.remove(key);
        }
        graph.dict.try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Outlines".into())),
        )?;
        if let (Some(first), Some(last)) = (graph.children.first(), graph.children.last()) {
            graph.dict.try_insert("First".into(), reference(*first))?;
            graph.dict.try_insert("Last".into(), reference(*last))?;
        }
        graph.dict.try_insert(
            "Count".into(),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(visible.to_string().as_bytes())?),
        )?;
        self.update_object(root_id, PdfObject::PdfDict(graph.dict))?;
        let mut root = self.ctx().get_root_owned()?;
        root.try_insert("Outlines".into(), reference(root_id))?;
        self.update_object(
            required(self.ctx().trailer().get("Root"))?,
            PdfObject::PdfDict(root),
        )
    }
}
