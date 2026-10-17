//! Explicit relationship policies for page transfers and deletion.
use super::{
    PdfDocument,
    destination::Destination,
    outlines::OutlineTarget,
    page_labels::{PageLabelRange, PageLabelStyle},
    resource_import::Importer,
};
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek},
};

/// Names are never silently merged. A nonempty prefix changes each imported
/// root field to `prefix_original` and each named destination to `prefix_original`.
#[derive(Debug, Clone)]
pub struct PageTransferOptions {
    pub name_prefix: String,
    /// Import the complete source outline/destination set; all its targets must
    /// be selected. False leaves the catalog navigation behind, but resolves
    /// named page links to explicit destinations so links remain usable.
    pub include_navigation: bool,
    pub preserve_labels: bool,
}
impl Default for PageTransferOptions {
    fn default() -> Self {
        Self {
            name_prefix: String::new(),
            include_navigation: false,
            preserve_labels: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PageDeletionPolicy {
    /// Default: reject only if navigation actually targets the removed page.
    RejectReferenced,
    /// Retarget navigation to another pre-deletion page, retaining its view.
    Retarget { page_index: u32 },
}

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn unsupported(message: &str) -> PdfError {
    PdfError::UnsupportedFeature(message.into())
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(crate) fn page_relationship_guard(&self) -> PdfResult<()> {
        let root = self.ctx().get_root_owned()?;
        let structured=self.validate_structure_relationships()?;
        for key in ["AA", "OpenAction", "Dests", "Threads", "OCProperties"] {
            if root.contains_key(key) {
                return Err(unsupported(&format!(
                    "page relationships with catalog {key}"
                )));
            }
        }
        if let Some(names) = root.get("Names") {
            let names = self
                .ctx()
                .resolve(names)?
                .into_dict()
                .ok_or_else(|| unsupported("Names dictionary"))?;
            if names.contains_key("JavaScript") {
                return Err(unsupported("page relationships with JavaScript name tree"));
            }
        }
        self.validate_annotation_ownership()?;
        self.form_field_tree()?;
        let mut total = 0usize;
        for (index, page) in self.collect_page_entries()?.iter().enumerate() {
            for key in ["AA", "B"] {
                if page.page_dict.contains_key(key) {
                    return Err(unsupported(&format!("page relationships with {key}")));
                }
            }
            if !structured && page.page_dict.contains_key("StructParents") {return Err(unsupported("page StructParents without supported tree"));}
            for record in self.page_annotations(index as u32)? {
                total += 1;
                if total > self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("page relationship objects".into()));
                }
                if record.annotation.is_none()
                    && !matches!(record.subtype.as_str(), "Link" | "Widget")
                {
                    return Err(unsupported("unknown annotation relationship graph"));
                }
                let handle = record
                    .handle
                    .ok_or_else(|| unsupported("direct annotation ownership"))?;
                let dict = self
                    .resolve_handle(&handle)?
                    .into_dict()
                    .ok_or_else(|| unsupported("annotation dictionary"))?;
                if let Some(flags) = dict.get("F") {
                    let flags = self.ctx().resolve(flags)?;
                    let flags = flags
                        .as_number()
                        .ok_or_else(|| unsupported("annotation F integer"))?
                        .as_u32_checked()?;
                    if flags & (64 | 128 | 512) != 0 {
                        return Err(unsupported("read-only or locked annotation relationship"));
                    }
                }
                for key in ["AA", "OC"] {
                    if dict.contains_key(key) {
                        return Err(unsupported(&format!("annotation relationship with {key}")));
                    }
                }
                if !structured && dict.contains_key("StructParent") {return Err(unsupported("annotation StructParent without supported tree"));}
                if dict.contains_key("A") && record.subtype != "Link" {
                    return Err(unsupported("non-Link annotation action"));
                }
                if record.subtype == "Link" {
                    self.navigation_destination(&dict)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_navigation_destination(
        &self,
        object: &PdfObject,
    ) -> PdfResult<Destination> {
        let value = self.ctx().resolve(object)?;
        let name = if let Some(s) = value.as_string() {
            Some(s.text_content_with_limits(&self.ctx().limits())?)
        } else {
            value.as_name().map(|n| n.name().to_string())
        };
        if let Some(name) = name {
            self.named_destination_views()?
                .into_iter()
                .find(|(n, _)| n == &name)
                .map(|(_, d)| d)
                .ok_or_else(|| unsupported("missing named target"))
        } else {
            self.decode_destination(&value)
        }
    }

    pub(crate) fn navigation_destination(&self, dict: &PdfDict) -> PdfResult<Option<Destination>> {
        if dict.contains_key("Dest") && dict.contains_key("A") {
            return Err(unsupported("navigation has Dest and A"));
        }
        if let Some(dest) = dict.get("Dest") {
            return self.resolve_navigation_destination(dest).map(Some);
        }
        if let Some(action) = dict.get("A") {
            let action = self
                .ctx()
                .resolve(action)?
                .into_dict()
                .ok_or_else(|| unsupported("action dictionary"))?;
            if action.contains_key("Next") {
                return Err(unsupported("chained navigation actions"));
            }
            match action
                .get("S")
                .and_then(PdfObject::as_name)
                .map(|n| n.name())
            {
                Some("GoTo") => {
                    return self
                        .resolve_navigation_destination(
                            action
                                .get("D")
                                .ok_or_else(|| unsupported("GoTo D missing"))?,
                        )
                        .map(Some);
                }
                Some("URI") => return Ok(None),
                _ => return Err(unsupported("unsupported navigation action")),
            }
        }
        Ok(None)
    }

    pub(super) fn prepare_page_relationships<S: Read + Seek>(
        &self,
        importer: &mut Importer<'_, S, R>,
        source_pages: &HashSet<ObjectId>,
        options: &PageTransferOptions,
    ) -> PdfResult<(Vec<ObjectId>, Option<PdfObject>)> {
        importer.source.page_relationship_guard()?;
        self.form_field_tree()?;
        let fields = importer
            .source
            .page_field_snapshot(source_pages, &options.name_prefix)?;
        importer.prepared.extend(fields.objects);
        for node in importer.source.form_field_tree()? {
            importer.blocked.insert(node.handle.object_id());
            importer
                .blocked
                .extend(node.widgets.into_iter().map(|w| w.handle.object_id()));
        }
        for (index, page) in importer.source.collect_page_entries()?.iter().enumerate() {
            importer.blocked.insert(page.page_id);
            if importer.source.identity() == self.identity()
                && !source_pages.contains(&page.page_id)
            {
                importer.mapped.insert(page.page_id, page.page_id);
            }
            for record in importer.source.page_annotations(index as u32)? {
                importer.blocked.insert(record.handle.unwrap().object_id());
            }
            if !source_pages.contains(&page.page_id) {
                continue;
            }
            for record in importer.source.page_annotations(index as u32)? {
                let id = record.handle.unwrap().object_id();
                self.ctx().require_annotation_permission()?;
                // Merged field/widgets already have a pruned/prefixed snapshot.
                let mut dict = importer
                    .prepared
                    .get(&id)
                    .cloned()
                    .unwrap_or(importer.source.resolve_object(id)?)
                    .into_dict()
                    .ok_or_else(|| unsupported("annotation dictionary"))?;
                dict.try_insert("P".into(), reference(page.page_id))?;
                if record.subtype == "Link" {
                    if let Some(destination) = importer.source.navigation_destination(&dict)? {
                        let encoded = importer.source.encode_destination(&destination)?;
                        if let Some(action) = dict.get("A") {
                            let mut action =
                                importer.source.ctx().resolve(action)?.into_dict().unwrap();
                            action.try_insert("D".into(), encoded)?;
                            dict.try_insert("A".into(), PdfObject::PdfDict(action))?;
                        } else {
                            dict.try_insert("Dest".into(), encoded)?;
                        }
                    }
                }
                importer.prepared.insert(id, PdfObject::PdfDict(dict));
            }
        }
        let resources = if fields.roots.is_empty() {
            None
        } else {
            fields
                .form
                .get("DR")
                .cloned()
                .map(|v| importer.object(v, 0))
                .transpose()?
        };
        let roots = fields
            .roots
            .into_iter()
            .map(|id| importer.reference(id, 0))
            .collect::<PdfResult<Vec<_>>>()?;
        Ok((roots, resources))
    }

    pub(crate) fn imported_page_labels<S: Read + Seek>(
        &self,
        source: &PdfDocument<S>,
        indices: &[u32],
        insertion: u32,
        old_count: u32,
        options: &PageTransferOptions,
    ) -> PdfResult<Option<Vec<PageLabelRange>>> {
        let original = self.labels_after_page_mapping(
            (0..insertion)
                .map(Some)
                .chain(std::iter::repeat_n(None, indices.len()))
                .chain((insertion..old_count).map(Some)),
        )?;
        let incoming = if options.preserve_labels {
            source.labels_after_page_mapping(indices.iter().copied().map(Some))?
        } else {
            None
        };
        let Some(incoming) = incoming else {
            return Ok(original);
        };
        let count = old_count as usize + indices.len();
        if count > self.ctx().limits().max_objects / 2 {
            return Err(PdfError::ResourceLimit("imported label ranges".into()));
        }
        let mut ranges = Vec::new();
        for index in 0..count as u32 {
            let imported = index >= insertion && index < insertion + indices.len() as u32;
            let (set, position) = if imported {
                (Some(&incoming), index - insertion)
            } else {
                (original.as_ref(), index)
            };
            let mut next = if let Some(range) =
                set.and_then(|r| r.iter().rev().find(|r| r.start_index <= position))
            {
                let mut next = range.clone();
                if next.style.is_some() {
                    next.start_number = next
                        .start_number
                        .checked_add(position - next.start_index)
                        .ok_or_else(|| PdfError::ResourceLimit("label overflow".into()))?;
                }
                next
            } else {
                PageLabelRange {
                    start_index: index,
                    style: Some(PageLabelStyle::Decimal),
                    prefix: String::new(),
                    start_number: index + 1,
                }
            };
            next.start_index = index;
            ranges.push(next);
        }
        Ok(Some(ranges))
    }

    pub(super) fn transfer_navigation<S: Read + Seek>(
        &self,
        importer: &mut Importer<'_, S, R>,
        options: &PageTransferOptions,
    ) -> PdfResult<()> {
        if !options.include_navigation {
            return Ok(());
        }
        let catalog = importer.source.ctx().get_root_owned()?;
        if let Some(outlines) = catalog.get("Outlines") {
            let dict = importer
                .source
                .ctx()
                .resolve(outlines)?
                .into_dict()
                .ok_or_else(|| unsupported("Outlines dictionary"))?;
            if (&dict)
                .into_iter()
                .any(|(key, _)| !matches!(key.as_str(), "Type" | "First" | "Last" | "Count"))
            {
                return Err(unsupported("merging unknown outline-root extensions"));
            }
        }
        let mapped = importer.mapped.clone();
        let same_document = importer.source.identity() == self.identity();
        let map = |destination: Destination| -> PdfResult<Destination> {
            let id = mapped.get(&destination.page.object_id()).ok_or_else(|| unsupported("unselected navigation target; select all targets or disable catalog navigation import"))?;
            if same_document && *id == destination.page.object_id() {
                return Err(unsupported("unselected catalog navigation target"));
            }
            Ok(Destination {
                page: self.object_handle(*id)?,
                view: destination.view,
            })
        };
        let name = |n: &str| {
            if options.name_prefix.is_empty() {
                n.to_string()
            } else {
                format!("{}_{n}", options.name_prefix)
            }
        };
        let mut existing: HashSet<_> = self
            .named_destination_views()?
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        let (names, tree, mut entries) = self.destination_entries()?;
        let (_, source_tree, incoming) = importer.source.destination_entries()?;
        if (&source_tree)
            .into_iter()
            .any(|(key, _)| !matches!(key.as_str(), "Names" | "Kids" | "Limits"))
        {
            return Err(unsupported(
                "merging unknown destination-tree root extensions",
            ));
        }
        if !incoming.is_empty() && self.effective_version()? < super::save_options::PdfVersion::V1_4
        {
            return Err(unsupported(
                "named destination tree writing requires PDF 1.4",
            ));
        }
        for (key, value) in incoming {
            let destination = importer.source.decode_destination(&value)?;
            map(destination.clone())?;
            let resolved = importer.source.ctx().resolve(&value)?;
            let value = if let PdfObject::PdfDict(mut dict) = resolved {
                dict.try_insert(
                    "D".into(),
                    importer.source.encode_destination(&destination)?,
                )?;
                PdfObject::PdfDict(dict)
            } else {
                importer.source.encode_destination(&destination)?
            };
            let key = name(&key.text_content_with_limits(&importer.source.ctx().limits())?);
            if !existing.insert(key.clone()) {
                return Err(unsupported(
                    "named destination collision; supply a unique name_prefix",
                ));
            }
            entries.push((
                crate::objects::pdf_string::PdfString::from_text_with_limits(
                    &key,
                    &self.ctx().limits(),
                )?,
                importer.object(value, 0)?,
            ));
            if entries.len() > self.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit(
                    "imported named destinations".into(),
                ));
            }
        }
        if !entries.is_empty() {
            self.write_destination_entries(names, tree, entries)?;
        }
        let mut outlines = HashMap::new();
        let items = importer.source.outline_items()?;
        for item in items {
            let original = importer
                .source
                .resolve_handle(&item.handle)?
                .into_dict()
                .unwrap();
            importer.source.navigation_destination(&original)?;
            let target = match item.target {
                OutlineTarget::Destination(dest) => OutlineTarget::Destination(map(dest)?),
                OutlineTarget::Named(key) => OutlineTarget::Named(name(&key)),
            };
            let parent = item
                .parent
                .as_ref()
                .map(|p| {
                    outlines
                        .get(&p.object_id())
                        .ok_or_else(|| unsupported("outline parent missing"))
                })
                .transpose()?;
            let handle = self.add_outline(parent, &item.title, &target)?;
            self.update_outline(&handle, &item.title, &target, item.open)?;
            let source = importer
                .source
                .resolve_handle(&item.handle)?
                .into_dict()
                .unwrap();
            let mut result = self.resolve_handle(&handle)?.into_dict().unwrap();
            for (key, value) in &source {
                if !matches!(
                    key.as_str(),
                    "Parent"
                        | "Prev"
                        | "Next"
                        | "First"
                        | "Last"
                        | "Count"
                        | "Dest"
                        | "A"
                        | "Title"
                ) {
                    result.try_insert(key.clone(), importer.object(value.clone(), 0)?)?;
                }
            }
            if let Some(action) = source.get("A") {
                let mut action = importer.source.ctx().resolve(action)?.into_dict().unwrap();
                let destination = importer
                    .source
                    .navigation_destination(&source)?
                    .ok_or_else(|| unsupported("outline destination missing"))?;
                action.try_insert(
                    "D".into(),
                    importer.source.encode_destination(&destination)?,
                )?;
                result.remove("Dest");
                result.try_insert("A".into(), importer.object(PdfObject::PdfDict(action), 0)?)?;
            }
            self.update_object(handle.object_id(), PdfObject::PdfDict(result))?;
            outlines.insert(item.handle.object_id(), handle);
        }
        Ok(())
    }

    pub fn delete_page_with_policy(&self, page: u32, policy: PageDeletionPolicy) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        self.ctx().require_content_edit_permission()?;
        self.edit_transaction(|doc| {
            doc.page_relationship_guard()?;
            let removed = doc.page_location(page)?.page_id;
            let structure_plan=doc.plan_structure_page_deletion(removed)?;
            if !doc.page_annotations(page)?.is_empty() {
                doc.ctx().require_annotation_permission()?;
            }
            doc.page_field_snapshot(&HashSet::from([removed]), "")?;
            let replacement = match policy {
                PageDeletionPolicy::RejectReferenced => None,
                PageDeletionPolicy::Retarget { page_index } => {
                    let id = doc.page_location(page_index)?.page_id;
                    if id == removed {
                        return Err(unsupported("retarget page is being deleted"));
                    }
                    Some(doc.object_handle(id)?)
                }
            };
            let mut objects = Vec::new();
            for (index, _) in doc.collect_page_entries()?.iter().enumerate() {
                if index as u32 == page {
                    continue;
                }
                for link in doc.page_link_handles(index as u32)? {
                    objects.push(link);
                }
            }
            for item in doc.outline_items()? {
                objects.push(item.handle);
            }
            for handle in objects {
                let mut dict = doc.resolve_handle(&handle)?.into_dict().unwrap();
                if let Some(mut dest) = doc.navigation_destination(&dict)? {
                    if dest.page.object_id() != removed {
                        continue;
                    }
                    dest.page = replacement.clone().ok_or_else(|| {
                        unsupported("page is referenced; supply PageDeletionPolicy::Retarget")
                    })?;
                    let value = doc.encode_destination(&dest)?;
                    if let Some(action) = dict.get("A") {
                        let mut action = doc.ctx().resolve(action)?.into_dict().unwrap();
                        action.try_insert("D".into(), value)?;
                        dict.try_insert("A".into(), PdfObject::PdfDict(action))?;
                    } else {
                        dict.try_insert("Dest".into(), value)?;
                    }
                    doc.update_object(handle.object_id(), PdfObject::PdfDict(dict))?;
                }
            }
            let (names, tree, mut entries) = doc.destination_entries()?;
            let mut changed = false;
            for (_, value) in &mut entries {
                let mut dest = doc.decode_destination(value)?;
                if dest.page.object_id() != removed {
                    continue;
                }
                dest.page = replacement
                    .clone()
                    .ok_or_else(|| unsupported("named destination references removed page"))?;
                let encoded = doc.encode_destination(&dest)?;
                *value = if let PdfObject::PdfDict(mut dict) = doc.ctx().resolve(value)? {
                    dict.try_insert("D".into(), encoded)?;
                    PdfObject::PdfDict(dict)
                } else {
                    encoded
                };
                changed = true;
            }
            if changed {
                doc.write_destination_entries(names, tree, entries)?;
            }
            // All widgets on the page disappear together; surviving logical
            // fields retain V, DA, AP and the identities of their other widgets.
            doc.detach_page_fields(page)?;
            doc.delete_page_inner(page)?;
            doc.apply_structure_page_deletion(structure_plan)?;
            doc.verify_no_live_page_reference(removed)?;
            doc.form_field_tree()?;
            doc.form_fields()?;
            doc.validate_annotation_ownership()?;
            Ok(())
        })
    }

    // Scan only the live catalog graph. Detached historical objects are not
    // owners; an unknown live back-reference to the removed page is refused.
    fn verify_no_live_page_reference(&self, removed: ObjectId) -> PdfResult<()> {
        let limits = self.ctx().limits();
        let mut pending = vec![(PdfObject::PdfDict(self.ctx().get_root_owned()?), 0usize)];
        let mut seen = HashSet::new();
        let mut nodes = 0usize;
        while let Some((object, depth)) = pending.pop() {
            nodes += 1;
            if nodes > limits.max_objects || depth > limits.max_depth {
                return Err(PdfError::ResourceLimit(
                    "page deletion live reference graph".into(),
                ));
            }
            match object {
                PdfObject::PdfReference(r) => {
                    if r.id() == removed {
                        return Err(unsupported("unknown live reference to removed page"));
                    }
                    if seen.insert(r.id()) {
                        pending.push((self.resolve_object(r.id())?, depth + 1));
                    }
                }
                PdfObject::PdfArray(array) => {
                    if array.len() > limits.max_objects.saturating_sub(nodes + pending.len()) {
                        return Err(PdfError::ResourceLimit("page deletion graph array".into()));
                    }
                    pending.extend((&array).into_iter().cloned().map(|v| (v, depth + 1)));
                }
                PdfObject::PdfDict(dict) => {
                    if (&dict).into_iter().len()
                        > limits.max_objects.saturating_sub(nodes + pending.len())
                    {
                        return Err(PdfError::ResourceLimit(
                            "page deletion graph dictionary".into(),
                        ));
                    }
                    pending.extend((&dict).into_iter().map(|(_, v)| (v.clone(), depth + 1)));
                }
                PdfObject::PdfStream(stream) => {
                    pending.push((PdfObject::PdfDict(stream.dict().clone()), depth + 1))
                }
                _ => {}
            }
        }
        Ok(())
    }
}
