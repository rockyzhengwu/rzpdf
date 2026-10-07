use crate::{
    error::{PdfError, PdfResult},
    objects::{object_id::ObjectId, pdf_dict::PdfDict, PdfObject},
    pdf_context::PDFContext,
};
use std::{
    collections::HashSet,
    io::{Read, Seek},
};

#[derive(Debug)]
pub struct PageTree;

#[cfg(test)]
mod budget_tests {
    use super::*;
    use crate::{
        budget::OperationBudget, document::owned::Document, objects::pdf_reference::PdfReference,
    };

    #[test]
    fn budgeted_page_location_validates_whole_tree_and_retains_target() {
        let doc = Document::new().unwrap();
        for index in 0..3 {
            doc.add_page(index).unwrap();
        }
        let normal = PageTree::entries(doc.ctx()).unwrap();
        for index in 0..3 {
            let pool = OperationBudget::new(1024 * 1024);
            let owned = PageTree::page_at_with_budget(doc.ctx(), index, &pool).unwrap();
            let expected = &normal[index as usize];
            assert_eq!(owned.entry.page_id, expected.page_id);
            assert_eq!(owned.entry.page_dict, expected.page_dict);
            assert_eq!(owned.entry.parent_id, expected.parent_id);
            assert_eq!(owned.entry.kid_index, expected.kid_index);
            assert_eq!(owned.entry.ancestor_ids, expected.ancestor_ids);
            assert_eq!(pool.live_bytes(), owned._lease.bytes());
            assert!(
                pool.live_bytes()
                    > owned.entry.ancestor_ids.capacity() * std::mem::size_of::<ObjectId>()
            );
            let peak = pool.peak_bytes();
            drop(owned);
            assert_eq!(pool.live_bytes(), 0);
            let small = OperationBudget::new(peak - 1);
            assert!(matches!(
                PageTree::page_at_with_budget(doc.ctx(), index, &small),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(small.live_bytes(), 0);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _owned = PageTree::page_at_with_budget(doc.ctx(), index, &pool).unwrap();
                panic!("page location owner unwind");
            }));
            assert!(caught.is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
        let pool = OperationBudget::new(1024 * 1024);
        assert!(matches!(
            PageTree::page_at_with_budget(doc.ctx(), 3, &pool),
            Err(PdfError::PageNotExist)
        ));
        assert_eq!(pool.live_bytes(), 0);
        // A later invalid sibling must still reject a query for page zero.
        let last = &normal[2];
        doc.update_object(last.page_id, PdfObject::PdfNull).unwrap();
        assert!(PageTree::entries(doc.ctx()).is_err());
        assert!(PageTree::page_at_with_budget(doc.ctx(), 0, &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(last.page_id, PdfObject::PdfDict(last.page_dict.clone()))
            .unwrap();
        let mut branch = doc
            .resolve_object(last.parent_id)
            .unwrap()
            .into_dict()
            .unwrap();
        branch.insert(
            "Count".into(),
            PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(99.0)),
        );
        doc.update_object(last.parent_id, PdfObject::PdfDict(branch))
            .unwrap();
        assert!(PageTree::entries(doc.ctx()).is_err());
        assert!(PageTree::page_at_with_budget(doc.ctx(), 0, &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn inheritance_budget_releases_missing_invalid_and_depth_limited_queries() {
        use crate::parser::options::{Limits, ParseOptions};
        let doc = Document::new_with_options(ParseOptions {
            limits: Limits {
                max_depth: 2,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let pool = OperationBudget::new(65536);
        let neighbor = pool.reserve(7).unwrap();
        assert!(PageTree::inherited_attribute_with_budget(
            doc.ctx(),
            &PdfObject::PdfDict(PdfDict::default()),
            "Resources",
            &pool
        )
        .unwrap()
        .is_none());
        assert_eq!(pool.live_bytes(), 7);
        assert!(matches!(
            PageTree::inherited_attribute_with_budget(
                doc.ctx(),
                &PdfObject::PdfNull,
                "Resources",
                &pool
            ),
            Err(PdfError::PageParentIsNotDict)
        ));
        assert_eq!(pool.live_bytes(), 7);
        let mut root = PdfDict::default();
        root.insert("Resources".into(), PdfObject::PdfNull);
        let root_id = doc.try_insert_object(PdfObject::PdfDict(root)).unwrap();
        let mut parent = PdfDict::default();
        parent.insert(
            "Parent".into(),
            PdfObject::PdfReference(PdfReference::from_id(root_id)),
        );
        let parent_id = doc.try_insert_object(PdfObject::PdfDict(parent)).unwrap();
        let mut child = PdfDict::default();
        child.insert(
            "Parent".into(),
            PdfObject::PdfReference(PdfReference::from_id(parent_id)),
        );
        let child = PdfObject::PdfDict(child);
        assert!(matches!(
            PageTree::inherited_attribute(doc.ctx(), &child, "Resources"),
            Err(PdfError::DocumentError(_))
        ));
        assert!(matches!(
            PageTree::inherited_attribute_with_budget(doc.ctx(), &child, "Resources", &pool),
            Err(PdfError::DocumentError(_))
        ));
        assert_eq!(pool.live_bytes(), 7);
        let node = PdfObject::PdfReference(PdfReference::from_id(root_id));
        let (value, lease) =
            PageTree::inherited_attribute_with_budget(doc.ctx(), &node, "Resources", &pool)
                .unwrap()
                .unwrap();
        assert_eq!(value, PdfObject::PdfNull);
        drop(value);
        drop(lease);
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn inherited_resource_budget_matches_legacy_and_releases_on_cycle() {
        let doc = Document::new().unwrap();
        let mut resources = PdfDict::default();
        resources.insert("Custom".into(), PdfObject::PdfNull);
        let mut parent = PdfDict::default();
        parent.insert("Resources".into(), PdfObject::PdfDict(resources));
        let id = doc.try_insert_object(PdfObject::PdfDict(parent)).unwrap();
        let mut child = PdfDict::default();
        child.insert(
            "Parent".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
        let child = PdfObject::PdfDict(child);
        let expected = PageTree::inherited_attribute(doc.ctx(), &child, "Resources")
            .unwrap()
            .unwrap();
        let probe = OperationBudget::new(65536);
        let (output, lease) =
            PageTree::inherited_attribute_with_budget(doc.ctx(), &child, "Resources", &probe)
                .unwrap()
                .unwrap();
        assert_eq!(output, expected);
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        drop(output);
        drop(lease);
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(
            PageTree::inherited_attribute_with_budget(doc.ctx(), &child, "Resources", &pool)
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (output, lease) =
            PageTree::inherited_attribute_with_budget(doc.ctx(), &child, "Resources", &pool)
                .unwrap()
                .unwrap();
        assert_eq!(output, expected);
        drop(output);
        drop(lease);
        let mut cycle = PdfDict::default();
        cycle.insert(
            "Parent".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
        doc.update_object(id, PdfObject::PdfDict(cycle)).unwrap();
        assert!(
            PageTree::inherited_attribute_with_budget(doc.ctx(), &child, "Resources", &probe)
                .is_err()
        );
        assert_eq!(probe.live_bytes(), 0);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PageTreeEntry {
    pub page_id: ObjectId,
    pub page_dict: PdfDict,
    pub parent_id: ObjectId,
    pub kid_index: usize,
    pub ancestor_ids: Vec<ObjectId>,
}

/// Data precedes its lease so the allocation is released first.
pub(crate) struct BudgetedPageTreeEntry {
    entry: PageTreeEntry,
    _lease: crate::budget::MemoryLease,
}

impl BudgetedPageTreeEntry {
    pub(crate) fn into_parts(self) -> (PageTreeEntry, crate::budget::MemoryLease) {
        (self.entry, self._lease)
    }
}

struct TreeVector<T> {
    data: Vec<T>,
    lease: crate::budget::MemoryLease,
}

impl<T> TreeVector<T> {
    fn new(pool: &crate::budget::OperationBudget) -> PdfResult<Self> {
        Ok(Self {
            data: Vec::new(),
            lease: pool.reserve(0)?,
        })
    }

    fn ensure_slot(
        &mut self,
        pool: &crate::budget::OperationBudget,
        maximum: usize,
    ) -> PdfResult<()> {
        if self.data.len() >= maximum {
            return Err(PdfError::ResourceLimit(
                "page tree resource limit exceeded".into(),
            ));
        }
        if self.data.len() < self.data.capacity() {
            return Ok(());
        }
        let capacity = self.data.capacity().saturating_mul(2).max(1).min(maximum);
        let bytes = capacity
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| PdfError::ResourceLimit("page tree vector size".into()))?;
        let mut lease = pool.reserve(bytes)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("page tree vector allocation".into()))?;
        lease.resize(
            replacement
                .capacity()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| PdfError::ResourceLimit("page tree vector capacity".into()))?,
        )?;
        replacement.append(&mut self.data);
        self.data = replacement;
        self.lease = lease;
        Ok(())
    }
}

struct BudgetedTreeWalk<'a> {
    pool: &'a crate::budget::OperationBudget,
    ancestors: TreeVector<ObjectId>,
    visited: TreeVector<ObjectId>,
    target_index: usize,
    pages: usize,
    target: Option<BudgetedPageTreeEntry>,
}

impl BudgetedTreeWalk<'_> {
    fn visit(&mut self, id: ObjectId, maximum: usize) -> PdfResult<()> {
        self.visited.ensure_slot(self.pool, maximum)?;
        let position = self
            .visited
            .data
            .binary_search_by_key(&(id.obj_num(), id.gen_num()), |id| {
                (id.obj_num(), id.gen_num())
            })
            .map_or_else(
                |position| Ok(position),
                |_| {
                    Err(PdfError::DocumentError(format!(
                        "cycle or duplicate page tree node: {id:?}"
                    )))
                },
            )?;
        self.visited.data.insert(position, id);
        Ok(())
    }

    fn walk<R: Read + Seek>(&mut self, ctx: &PDFContext<R>, id: ObjectId) -> PdfResult<usize> {
        let limits = ctx.limits();
        if self.ancestors.data.len() >= limits.max_depth {
            return Err(PdfError::ResourceLimit(
                "page tree resource limit exceeded".into(),
            ));
        }
        self.visit(id, limits.max_objects)?;
        let (_dict_lease, dict) = {
            let (object, lease) = ctx.resolve_owned_with_budget(
                &PdfObject::PdfReference(crate::objects::pdf_reference::PdfReference::from_id(id)),
                self.pool,
            )?;
            (
                lease,
                object.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("page tree node is not a dictionary".into())
                })?,
            )
        };
        if dict
            .get("Type")
            .and_then(PdfObject::as_name)
            .map(|name| name.name())
            != Some("Pages")
        {
            return Err(PdfError::DocumentError(
                "page tree branch must have Type /Pages".into(),
            ));
        }
        let kids = dict
            .get("Kids")
            .and_then(PdfObject::as_array)
            .ok_or_else(|| PdfError::DocumentError("Pages Kids must be an array".into()))?;
        self.ancestors.ensure_slot(self.pool, limits.max_depth)?;
        self.ancestors.data.push(id);
        let mut count = 0usize;
        for (kid_index, kid) in kids.into_iter().enumerate() {
            if self.visited.data.len() >= limits.max_objects {
                return Err(PdfError::ResourceLimit(
                    "page tree resource limit exceeded".into(),
                ));
            }
            let kid_id = kid
                .as_reference()
                .ok_or_else(|| {
                    PdfError::DocumentError("page tree Kids must be indirect references".into())
                })?
                .id();
            let (mut kid_lease, kid_dict) = {
                let (object, lease) = ctx.resolve_owned_with_budget(kid, self.pool)?;
                (
                    lease,
                    object.into_dict().ok_or_else(|| {
                        PdfError::DocumentError("page tree kid must be a dictionary".into())
                    })?,
                )
            };
            match kid_dict
                .get("Type")
                .and_then(PdfObject::as_name)
                .map(|name| name.name())
            {
                Some("Pages") => {
                    count = count
                        .checked_add(self.walk(ctx, kid_id)?)
                        .ok_or_else(|| PdfError::ResourceLimit("page tree count".into()))?;
                }
                Some("Page") => {
                    self.visit(kid_id, limits.max_objects)?;
                    if self.pages == self.target_index {
                        let length = self.ancestors.data.len();
                        let mut lease = self.pool.reserve(
                            length
                                .checked_mul(std::mem::size_of::<ObjectId>())
                                .ok_or_else(|| {
                                    PdfError::ResourceLimit("page ancestors size".into())
                                })?,
                        )?;
                        let mut ancestors = Vec::new();
                        ancestors.try_reserve_exact(length).map_err(|_| {
                            PdfError::ResourceLimit("page ancestors allocation".into())
                        })?;
                        lease.resize(
                            ancestors
                                .capacity()
                                .checked_mul(std::mem::size_of::<ObjectId>())
                                .ok_or_else(|| {
                                    PdfError::ResourceLimit("page ancestors capacity".into())
                                })?,
                        )?;
                        ancestors.extend_from_slice(&self.ancestors.data);
                        kid_lease.absorb(&mut lease)?;
                        self.target = Some(BudgetedPageTreeEntry {
                            entry: PageTreeEntry {
                                page_id: kid_id,
                                page_dict: kid_dict,
                                parent_id: id,
                                kid_index,
                                ancestor_ids: ancestors,
                            },
                            _lease: kid_lease,
                        });
                    }
                    self.pages = self
                        .pages
                        .checked_add(1)
                        .ok_or_else(|| PdfError::ResourceLimit("page tree count".into()))?;
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| PdfError::ResourceLimit("page tree count".into()))?;
                }
                _ => {
                    return Err(PdfError::DocumentError(
                        "page tree node missing valid Type".into(),
                    ))
                }
            }
        }
        self.ancestors.data.pop();
        let declared = dict
            .get("Count")
            .and_then(PdfObject::as_number)
            .ok_or_else(|| PdfError::DocumentError("Pages Count must be an integer".into()))?
            .as_u64_checked()?;
        if declared != count as u64 {
            return Err(PdfError::DocumentError(format!(
                "incorrect Pages Count for {id:?}: declared {declared}, found {count}"
            )));
        }
        Ok(count)
    }
}

impl PageTree {
    /// Validate the entire tree but retain only the requested page. Owned
    /// dictionaries and traversal vectors reserve before allocation.
    pub(crate) fn page_at_with_budget<R: Read + Seek>(
        ctx: &PDFContext<R>,
        page: u32,
        pool: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedPageTreeEntry> {
        let id = {
            let trailer = ctx.trailer();
            let root = trailer
                .get("Root")
                .ok_or_else(|| PdfError::DocumentError("Root not found".into()))?;
            let (_root_lease, root) = {
                let (root, lease) = ctx.resolve_owned_with_budget(root, pool)?;
                (
                    lease,
                    root.into_dict()
                        .ok_or_else(|| PdfError::DocumentError("Root is not a Dict".into()))?,
                )
            };
            root.get("Pages")
                .and_then(PdfObject::as_reference)
                .ok_or_else(|| {
                    PdfError::DocumentError("Pages root must be an indirect reference".into())
                })?
                .id()
        };
        let mut walk = BudgetedTreeWalk {
            pool,
            ancestors: TreeVector::new(pool)?,
            visited: TreeVector::new(pool)?,
            target_index: page as usize,
            pages: 0,
            target: None,
        };
        walk.walk(ctx, id)?;
        walk.target.take().ok_or(PdfError::PageNotExist)
    }

    pub(crate) fn inherited_attribute_with_budget<R: Read + Seek>(
        ctx: &PDFContext<R>,
        node: &PdfObject,
        name: &str,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<Option<(PdfObject, crate::budget::MemoryLease)>> {
        let limits = ctx.limits();
        let size = std::mem::size_of::<ObjectId>();
        let mut visited_lease = budget.reserve(
            limits
                .max_depth
                .checked_mul(size)
                .ok_or_else(|| PdfError::ResourceLimit("inheritance visited table".into()))?,
        )?;
        let mut visited = Vec::new();
        visited
            .try_reserve_exact(limits.max_depth)
            .map_err(|_| PdfError::ResourceLimit("inheritance visited allocation".into()))?;
        visited_lease.resize(
            visited
                .capacity()
                .checked_mul(size)
                .ok_or_else(|| PdfError::ResourceLimit("inheritance visited capacity".into()))?,
        )?;
        let mut current = crate::objects::clone_object_with_budget(node, budget, limits, 0)?;
        for _ in 0..limits.max_depth {
            if let Some(reference) = current.0.as_reference() {
                if visited.contains(&reference.id()) {
                    return Err(PdfError::DocumentError("cycle in page inheritance".into()));
                }
                visited.push(reference.id());
            }
            let (object, _dict_lease) = ctx.resolve_owned_with_budget(&current.0, budget)?;
            current = (PdfObject::PdfNull, budget.reserve(0)?);
            let dict = object.into_dict().ok_or(PdfError::PageParentIsNotDict)?;
            if let Some(value) = dict.get(name) {
                return ctx.resolve_owned_with_budget(value, budget).map(Some);
            }
            match dict.get("Parent") {
                Some(parent) => {
                    current = crate::objects::clone_object_with_budget(parent, budget, limits, 0)?
                }
                None => return Ok(None),
            }
        }
        Err(PdfError::DocumentError(
            "page inheritance depth limit exceeded".into(),
        ))
    }

    pub fn inherited_attribute<R: Read + Seek>(
        ctx: &PDFContext<R>,
        node: &PdfObject,
        name: &str,
    ) -> PdfResult<Option<PdfObject>> {
        let mut current = node.clone();
        let mut visited = HashSet::new();
        for _ in 0..ctx.limits().max_depth {
            if let Some(reference) = current.as_reference() {
                if !visited.insert(reference.id()) {
                    return Err(PdfError::DocumentError("cycle in page inheritance".into()));
                }
            }
            let dict = ctx
                .resolve_owned(&current)?
                .into_dict()
                .ok_or(PdfError::PageParentIsNotDict)?;
            if let Some(value) = dict.get(name) {
                return ctx.resolve_owned(value).map(Some);
            }
            match dict.get("Parent") {
                Some(parent) => current = parent.clone(),
                None => return Ok(None),
            }
        }
        Err(PdfError::DocumentError(
            "page inheritance depth limit exceeded".into(),
        ))
    }

    pub(crate) fn entries<R: Read + Seek>(ctx: &PDFContext<R>) -> PdfResult<Vec<PageTreeEntry>> {
        let root = ctx.get_root_owned()?;
        let id = root
            .get("Pages")
            .and_then(PdfObject::as_reference)
            .ok_or_else(|| {
                PdfError::DocumentError("Pages root must be an indirect reference".into())
            })?
            .id();
        let mut entries = Vec::new();
        Self::walk(ctx, id, &mut Vec::new(), &mut HashSet::new(), &mut entries)?;
        Ok(entries)
    }

    fn walk<R: Read + Seek>(
        ctx: &PDFContext<R>,
        id: ObjectId,
        ancestors: &mut Vec<ObjectId>,
        visited: &mut HashSet<ObjectId>,
        entries: &mut Vec<PageTreeEntry>,
    ) -> PdfResult<()> {
        if ancestors.len() >= ctx.limits().max_depth || visited.len() >= ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit(
                "page tree resource limit exceeded".into(),
            ));
        }
        if !visited.insert(id) {
            return Err(PdfError::DocumentError(format!(
                "cycle or duplicate page tree node: {id:?}"
            )));
        }
        let dict = ctx.resolve_object_owned(id)?.into_dict().ok_or_else(|| {
            PdfError::DocumentError(format!("page tree node is not a dictionary: {id:?}"))
        })?;
        if dict
            .get("Type")
            .and_then(PdfObject::as_name)
            .map(|n| n.name())
            != Some("Pages")
        {
            return Err(PdfError::DocumentError(
                "page tree branch must have Type /Pages".into(),
            ));
        }
        let kids = dict
            .get("Kids")
            .and_then(PdfObject::as_array)
            .ok_or_else(|| PdfError::DocumentError("Pages Kids must be an array".into()))?;
        ancestors.push(id);
        let initial_count = entries.len();
        for (kid_index, kid) in kids.into_iter().enumerate() {
            if visited.len() >= ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit(
                    "page tree resource limit exceeded".into(),
                ));
            }
            let kid_id = kid
                .as_reference()
                .ok_or_else(|| {
                    PdfError::DocumentError("page tree Kids must be indirect references".into())
                })?
                .id();
            let kid_dict = ctx
                .resolve_object_owned(kid_id)?
                .into_dict()
                .ok_or_else(|| {
                    PdfError::DocumentError("page tree kid must be a dictionary".into())
                })?;
            match kid_dict
                .get("Type")
                .and_then(PdfObject::as_name)
                .map(|n| n.name())
            {
                Some("Pages") => Self::walk(ctx, kid_id, ancestors, visited, entries)?,
                Some("Page") => {
                    if !visited.insert(kid_id) {
                        return Err(PdfError::DocumentError(format!(
                            "duplicate page tree node: {kid_id:?}"
                        )));
                    }
                    entries.push(PageTreeEntry {
                        page_id: kid_id,
                        page_dict: kid_dict,
                        parent_id: id,
                        kid_index,
                        ancestor_ids: ancestors.clone(),
                    });
                }
                _ => {
                    return Err(PdfError::DocumentError(
                        "page tree node missing valid Type".into(),
                    ));
                }
            }
        }
        ancestors.pop();
        let declared_count = dict
            .get("Count")
            .and_then(PdfObject::as_number)
            .ok_or_else(|| PdfError::DocumentError("Pages Count must be an integer".into()))?
            .as_u64_checked()?;
        if declared_count != (entries.len() - initial_count) as u64 {
            return Err(PdfError::DocumentError(format!(
                "incorrect Pages Count for {id:?}: declared {declared_count}, found {}",
                entries.len() - initial_count
            )));
        }
        Ok(())
    }
}
