//! Explicit optional-content authoring. No actions, flattening or layer deletion.
use super::{
    PdfDocument,
    identity::{DocumentStamp, ObjectHandle},
    save_options::PdfVersion,
};
use crate::{
    budget::OperationBudget,
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_reference::PdfReference, pdf_string::PdfString,
    },
    page::content_program::ContentProgram,
};
use std::{
    collections::HashSet,
    io::{Read, Seek},
};

const MAX_GROUPS: usize = 1024;
const MAX_CONFIGS: usize = 256;
const MAX_NODES: usize = 4096;
const MAX_CONTENT: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum LayerIntent {
    View,
    Design,
}
#[derive(Debug, Clone, Copy)]
pub enum LayerBaseState {
    On,
    Off,
    Unchanged,
}
#[derive(Debug, Clone, Copy)]
pub enum LayerPolicy {
    AnyOn,
    AllOn,
    AnyOff,
    AllOff,
}
#[derive(Debug, Clone)]
pub enum LayerExpression {
    Group(ObjectHandle),
    And(Vec<LayerExpression>),
    Or(Vec<LayerExpression>),
    Not(Box<LayerExpression>),
}
/// Complete state fields for the selected simple configuration profile.
#[derive(Debug, Clone)]
pub struct LayerConfiguration {
    pub name: Option<String>,
    pub base: LayerBaseState,
    pub on: Vec<ObjectHandle>,
    pub off: Vec<ObjectHandle>,
}
/// Index is meaningful only at this exact document revision.
#[derive(Debug, Clone)]
pub struct LayerConfigurationTarget {
    stamp: DocumentStamp,
    index: Option<usize>,
}

fn invalid(s: &str) -> PdfError {
    PdfError::DocumentError(format!("layer edit: {s}"))
}
fn unsupported(s: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("layer edit: {s}"))
}
fn name(s: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(s.into()))
}
fn reference(h: &ObjectHandle) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(h.object_id()))
}
fn array(values: impl IntoIterator<Item = PdfObject>) -> PdfResult<PdfObject> {
    let mut a = PdfArray::default();
    for v in values {
        a.try_add_obj(v)?;
    }
    Ok(PdfObject::PdfArray(a))
}

impl<R: Read + Seek> PdfDocument<R> {
    fn validate_layer_content(
        &self,
        source: &[u8],
        new_segment: bool,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        if source.len()
            > MAX_CONTENT
                .min(self.ctx.limits().max_raw_stream_bytes)
                .min(self.ctx.limits().max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("layer content bytes".into()));
        }
        let text =
            std::str::from_utf8(source).map_err(|_| unsupported("binary content segment"))?;
        let mut builder = crate::page::content_builder::ContentBuilder::new();
        builder.raw_operator(text);
        let bytes = builder.try_build_with_limits(self.ctx.limits())?;
        let mut options = self.ctx.options();
        options.mode = crate::parser::syntax::ParseMode::Strict;
        let bytes = crate::budget::BudgetedBytes::copy_from_slice(budget, &bytes)?;
        let program_owner = ContentProgram::from_decoded_bytes_with_budget(bytes, options, budget)?;
        let program = program_owner.program();
        let mut marked = 0usize;
        let mut path = false;
        for op in program.operations() {
            let o = &op.operator;
            if new_segment && matches!(o.name(), "DP" | "MP") {
                return Err(unsupported("marked-content points in a new layer segment"));
            }
            match o.name() {
                "BMC" | "BDC" => {
                    marked += 1;
                    if marked > self.ctx.limits().max_depth {
                        return Err(PdfError::ResourceLimit("layer marked depth".into()));
                    }
                    if new_segment && o.name() == "BDC" {
                        let d = o
                            .operand(1)?
                            .as_dict()
                            .ok_or_else(|| unsupported("named properties in a new segment"))?;
                        if d.contains_key("MCID")
                            || !matches!(o.get_as_name(0)?.name(), "Span" | "Artifact")
                        {
                            return Err(unsupported("new MCID ownership or marked tag"));
                        }
                    }
                }
                "EMC" => {
                    marked = marked
                        .checked_sub(1)
                        .ok_or_else(|| invalid("EMC without marked content"))?;
                }
                "m" | "re" | "l" | "c" | "v" | "y" | "h" => path = true,
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => path = false,
                "BI" | "BX" | "EX" => {
                    return Err(unsupported(
                        "inline images and compatibility sections in layer segments",
                    ));
                }
                _ => {}
            }
        }
        if marked != 0 || path {
            return Err(invalid("unclosed marked content or pending path"));
        }
        Ok(())
    }
    fn validate_layer_membership(&self, h: &ObjectHandle) -> PdfResult<()> {
        self.validate_handle(h)?;
        let model = self
            .optional_content()?
            .ok_or_else(|| invalid("membership requires OCProperties"))?;
        if model.groups.len() > MAX_GROUPS.min(self.ctx.limits().max_objects)
            || model.configurations.len() > MAX_CONFIGS.min(self.ctx.limits().max_objects)
        {
            return Err(PdfError::ResourceLimit("layer registration count".into()));
        }
        let object = self.resolve_handle(h)?;
        let d = object
            .as_dict()
            .ok_or_else(|| invalid("membership dictionary"))?;
        let kind = d.get("Type").and_then(PdfObject::as_name).map(|n| n.name());
        if !matches!(kind, Some("OCG" | "OCMD")) {
            return Err(invalid("membership Type"));
        }
        let selection = self.optional_content_visibility(&Default::default())?;
        selection.is_visible(self, &reference(h))?;
        Ok(())
    }
    fn register_layer_property(
        &self,
        page: u32,
        h: &ObjectHandle,
        budget: &OperationBudget,
    ) -> PdfResult<String> {
        let (location, _location_lease) =
            self.page_location_with_budget(page, budget)?.into_parts();
        let page_owner = self.get_page_with_budget(page, budget)?;
        let (mut resources, _resources_lease) = page_owner.resource_with_budget(budget)?;
        let (mut properties, _properties_lease) = match resources.get("Properties") {
            Some(v) => {
                let (o, l) = self.ctx.resolve_owned_with_budget(v, budget)?;
                (
                    o.into_dict()
                        .ok_or_else(|| invalid("Properties dictionary"))?,
                    Some(l),
                )
            }
            None => (PdfDict::default(), None),
        };
        if (&properties).into_iter().count() >= MAX_NODES.min(self.ctx.limits().max_objects) {
            return Err(PdfError::ResourceLimit("layer Properties".into()));
        }
        let label = (0..=MAX_NODES)
            .map(|i| format!("OCrz{i}"))
            .find(|s| !properties.contains_key(s))
            .ok_or_else(|| invalid("Properties names exhausted"))?;
        properties.try_insert(label.clone(), reference(h))?;
        resources.try_insert("Properties".into(), PdfObject::PdfDict(properties))?;
        let mut d = location.page_dict;
        d.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        self.update_object(location.page_id, PdfObject::PdfDict(d))?;
        Ok(label)
    }
    /// Append a self-contained, UTF-8/ASCII-encoded content segment. Raw binary
    /// inline images, new MCIDs, and unbalanced old page programs are refused.
    pub fn append_layer_content(
        &self,
        page: u32,
        membership: &ObjectHandle,
        source: &[u8],
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        self.layer_guard(false, budget)?;
        let _input_scratch = self.layer_scratch(budget, source.len())?;
        let page_owner = self.get_page_with_budget(page, budget)?;
        let old_owner = page_owner.content_program_with_budget(budget)?;
        let old = old_owner.program();
        let _old_scratch = budget.reserve(
            old.source_bytes()
                .len()
                .checked_mul(16)
                .ok_or_else(|| invalid("content size overflow"))?,
        )?;
        self.validate_layer_membership(membership)?;
        self.validate_layer_content(old.source_bytes(), false, budget)?;
        self.validate_layer_content(source, true, budget)?;
        self.edit_transaction_with_budget(budget, |doc| {
            let property = doc.register_layer_property(page, membership, budget)?;
            let mut bytes = format!("q\n/OC /{property} BDC\n").into_bytes();
            bytes.extend_from_slice(source);
            bytes.extend_from_slice(b"\nEMC\nQ\n");
            doc.append_page_contents_inner(page, bytes)?;
            let selection = doc.optional_content_visibility(&Default::default())?;
            doc.get_page(page)?
                .display_list_with_optional_content(&selection)?;
            Ok(())
        })
    }
    /// Add a marked-content wrapper to exactly one top-level Form/image Do.
    /// The XObject itself is never mutated or cloned, preserving its sharing.
    pub fn bind_layer_xobject_call(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        membership: &ObjectHandle,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        self.layer_guard(false, budget)?;
        let _scratch = self.layer_scratch(budget, expected.source_bytes().len())?;
        self.validate_layer_membership(membership)?;
        if !expected
            .source_stamp()
            .is_some_and(|s| self.is_stamp_current(s))
            || expected.source_page_id != Some(self.page_location(page)?.page_id)
        {
            return Err(invalid("foreign or stale content snapshot"));
        }
        self.validate_layer_content(expected.source_bytes(), false, budget)?;
        let op = expected
            .operations()
            .get(operation)
            .ok_or_else(|| invalid("operation index"))?;
        if op.operator.name() != "Do" || op.operator.num_operands() != 1 {
            return Err(invalid("named Do required"));
        }
        let object = self
            .get_page(page)?
            .get_xobject(op.operator.get_as_name(0)?.name())?;
        let stream = object
            .as_stream()
            .ok_or_else(|| invalid("XObject stream"))?;
        let d = stream.dict();
        if !d
            .get("Subtype")
            .and_then(PdfObject::as_name)
            .is_some_and(|n| matches!(n.name(), "Form" | "Image"))
        {
            return Err(unsupported("XObject subtype"));
        }
        if ["OC", "StructParent", "StructParents"]
            .iter()
            .any(|k| d.contains_key(k))
        {
            return Err(unsupported(
                "XObject optional-content or structure ownership",
            ));
        }
        self.edit_transaction_with_budget(budget, |doc| {
            // The existing commit validates the snapshot before entering its
            // transaction. Resource registration occurs inside that edit closure.
            doc.commit_page_content_edit_with_budget(page, expected, budget, || {
                let property = doc.register_layer_property(page, membership, budget)?;
                let mut bytes = format!("/OC /{property} BDC\n").into_bytes();
                bytes.extend_from_slice(&expected.source_bytes()[op.lexical_range.clone()]);
                bytes.extend_from_slice(b"\nEMC\n");
                expected.with_replaced_operation_sequences_with_budget(
                    &[crate::page::content_program::OperationReplacement {
                        index: operation,
                        source: &bytes,
                        operation_count: 3,
                    }],
                    doc.ctx.options(),
                    budget,
                )
            })?;
            let selection = doc.optional_content_visibility(&Default::default())?;
            doc.get_page(page)?
                .display_list_with_optional_content(&selection)?;
            Ok(())
        })
    }
    fn layer_guard(&self, ve: bool, budget: &OperationBudget) -> PdfResult<()> {
        self.ctx.require_unsigned_source_with_budget(budget)?;
        self.ctx.require_content_edit_permission()?;
        if self.effective_version()?
            < if ve {
                PdfVersion::V1_6
            } else {
                PdfVersion::V1_5
            }
        {
            return Err(unsupported("explicit PDF version upgrade required"));
        }
        Ok(())
    }
    // Conservative scratch allowance for bounded newly-built tables. Source
    // dictionary ownership and transaction snapshots have separate reservations.
    fn layer_scratch(
        &self,
        budget: &OperationBudget,
        extra: usize,
    ) -> PdfResult<crate::budget::MemoryLease> {
        budget.reserve(
            extra
                .checked_mul(16)
                .and_then(|n| n.checked_add(2 * 1024 * 1024))
                .ok_or_else(|| invalid("size overflow"))?,
        )
    }
    fn layer_properties(
        &self,
        budget: &OperationBudget,
    ) -> PdfResult<(
        PdfDict,
        PdfDict,
        crate::budget::MemoryLease,
        Option<crate::budget::MemoryLease>,
    )> {
        let root_ref = self
            .ctx
            .trailer()
            .get("Root")
            .cloned()
            .ok_or_else(|| invalid("Root missing"))?;
        let (root, root_lease) = self.ctx.resolve_owned_with_budget(&root_ref, budget)?;
        let root = root.into_dict().ok_or_else(|| invalid("Root dictionary"))?;
        let (props, lease) = match root.get("OCProperties") {
            None | Some(PdfObject::PdfNull) => {
                let mut d = PdfDict::default();
                d.try_insert("OCGs".into(), array([])?)?;
                let mut default = PdfDict::default();
                default.try_insert("BaseState".into(), name("ON"))?;
                d.try_insert("D".into(), PdfObject::PdfDict(default))?;
                (d, None)
            }
            Some(v) => {
                let (o, l) = self.ctx.resolve_owned_with_budget(v, budget)?;
                (
                    o.into_dict()
                        .ok_or_else(|| invalid("OCProperties dictionary"))?,
                    Some(l),
                )
            }
        };
        // Validate registration, without claiming that opaque configurations
        // are understood or reconstructing them from the read model.
        if self.optional_content()?.is_some_and(|m| {
            m.groups.len() > MAX_GROUPS.min(self.ctx.limits().max_objects)
                || m.configurations.len() > MAX_CONFIGS
        }) {
            return Err(PdfError::ResourceLimit("layer registration count".into()));
        }
        Ok((root, props, root_lease, lease))
    }
    fn store_layer_properties(&self, mut root: PdfDict, props: PdfDict) -> PdfResult<()> {
        let id = self
            .ctx
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| invalid("indirect Root required"))?;
        root.try_insert("OCProperties".into(), PdfObject::PdfDict(props))?;
        self.update_object(id, PdfObject::PdfDict(root))
    }
    fn registered_layer(&self, h: &ObjectHandle) -> PdfResult<()> {
        self.validate_handle(h)?;
        let m = self
            .optional_content()?
            .ok_or_else(|| invalid("no registered groups"))?;
        if m.groups.len() > MAX_GROUPS.min(self.ctx.limits().max_objects)
            || m.configurations.len() > MAX_CONFIGS.min(self.ctx.limits().max_objects)
        {
            return Err(PdfError::ResourceLimit("layer registration count".into()));
        }
        if !m.groups.iter().any(|g| g.handle == *h) {
            return Err(invalid("unregistered group"));
        }
        Ok(())
    }
    pub fn create_layer(
        &self,
        label: &str,
        intent: LayerIntent,
        budget: &OperationBudget,
    ) -> PdfResult<ObjectHandle> {
        self.layer_guard(false, budget)?;
        let _scratch = self.layer_scratch(budget, label.len())?;
        let label = PdfString::from_text_with_limits(label, &self.ctx.limits())?;
        let (root, mut props, _root_lease, _props_lease) = self.layer_properties(budget)?;
        let (groups, _lease) = self.ctx.resolve_owned_with_budget(
            props.get("OCGs").ok_or_else(|| invalid("OCGs missing"))?,
            budget,
        )?;
        let mut groups = groups.into_array().ok_or_else(|| invalid("OCGs array"))?;
        if groups.len() >= MAX_GROUPS.min(self.ctx.limits().max_objects) {
            return Err(PdfError::ResourceLimit("layer groups".into()));
        }
        self.edit_transaction_with_budget(budget, |doc| {
            let mut d = PdfDict::default();
            d.try_insert("Type".into(), name("OCG"))?;
            d.try_insert("Name".into(), PdfObject::PdfString(label))?;
            d.try_insert(
                "Intent".into(),
                name(match intent {
                    LayerIntent::View => "View",
                    LayerIntent::Design => "Design",
                }),
            )?;
            let id = doc.try_insert_object(PdfObject::PdfDict(d))?;
            let h = doc.object_handle(id)?;
            groups.try_add_obj(reference(&h))?;
            props.try_insert("OCGs".into(), PdfObject::PdfArray(groups))?;
            doc.store_layer_properties(root, props)?;
            Ok(h)
        })
    }
    pub fn rename_layer(
        &self,
        group: &ObjectHandle,
        label: &str,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        self.layer_guard(false, budget)?;
        let _scratch = self.layer_scratch(budget, label.len())?;
        self.registered_layer(group)?;
        let label = PdfString::from_text_with_limits(label, &self.ctx.limits())?;
        let (d, _lease) = self
            .ctx
            .resolve_owned_with_budget(&reference(group), budget)?;
        let mut d = d.into_dict().ok_or_else(|| invalid("group dictionary"))?;
        d.try_insert("Name".into(), PdfObject::PdfString(label))?;
        self.edit_transaction_with_budget(budget, |doc| {
            doc.update_handle(group, PdfObject::PdfDict(d))
        })
    }
    pub fn layer_configuration_target(
        &self,
        index: Option<usize>,
    ) -> PdfResult<LayerConfigurationTarget> {
        let m = self
            .optional_content()?
            .ok_or_else(|| invalid("no configurations"))?;
        if index.is_some_and(|i| i >= m.configurations.len()) {
            return Err(invalid("configuration index"));
        }
        Ok(LayerConfigurationTarget {
            stamp: self.stamp(),
            index,
        })
    }
    fn layer_config_dict(&self, c: &LayerConfiguration) -> PdfResult<PdfDict> {
        if c.on.len().saturating_add(c.off.len()) > MAX_GROUPS.min(self.ctx.limits().max_objects) {
            return Err(PdfError::ResourceLimit("configuration members".into()));
        }
        let mut seen = HashSet::new();
        for h in c.on.iter().chain(&c.off) {
            self.registered_layer(h)?;
            if !seen.insert(h.object_id()) {
                return Err(invalid("duplicate or conflicting state"));
            }
        }
        let mut d = PdfDict::default();
        if let Some(label) = &c.name {
            d.try_insert(
                "Name".into(),
                PdfObject::PdfString(PdfString::from_text_with_limits(label, &self.ctx.limits())?),
            )?;
        }
        d.try_insert(
            "BaseState".into(),
            name(match c.base {
                LayerBaseState::On => "ON",
                LayerBaseState::Off => "OFF",
                LayerBaseState::Unchanged => "Unchanged",
            }),
        )?;
        d.try_insert("ON".into(), array(c.on.iter().map(reference))?)?;
        d.try_insert("OFF".into(), array(c.off.iter().map(reference))?)?;
        Ok(d)
    }
    /// None appends an alternative; Some(target) patches its complete state fields.
    pub fn write_layer_configuration(
        &self,
        target: Option<&LayerConfigurationTarget>,
        c: &LayerConfiguration,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        self.layer_guard(false, budget)?;
        let _scratch = self.layer_scratch(budget, c.name.as_ref().map_or(0, String::len))?;
        if target.is_some_and(|t| !self.is_stamp_current(&t.stamp)) {
            return Err(invalid("foreign or stale configuration target"));
        }
        // ISO 32000-1 Table 101: default BaseState must be ON. Reading
        // tolerates some legacy defaults; authoring must not emit those.
        if target.is_some_and(|t| t.index.is_none()) && !matches!(c.base, LayerBaseState::On) {
            return Err(unsupported("default BaseState must be ON"));
        }
        let patch = self.layer_config_dict(c)?;
        let (root, mut props, _root_lease, _props_lease) = self.layer_properties(budget)?;
        let (configs, _configs_lease) = match props.get("Configs") {
            Some(v) => {
                let (o, l) = self.ctx.resolve_owned_with_budget(v, budget)?;
                (
                    o.into_array().ok_or_else(|| invalid("Configs array"))?,
                    Some(l),
                )
            }
            None => (PdfArray::default(), None),
        };
        let mut configs = configs;
        let old = match target {
            Some(t) => match t.index {
                Some(i) => Some(
                    configs
                        .get(i)
                        .ok_or_else(|| invalid("configuration index"))?,
                ),
                None => props.get("D"),
            },
            None => None,
        };
        let mut d = PdfDict::default();
        let mut old_lease = None;
        if let Some(old) = old {
            let (o, l) = self.ctx.resolve_owned_with_budget(old, budget)?;
            old_lease = Some(l);
            d = o
                .into_dict()
                .ok_or_else(|| invalid("configuration dictionary"))?;
            for (key, _) in &d {
                if !matches!(
                    key.as_str(),
                    "Name" | "Creator" | "BaseState" | "ON" | "OFF" | "Intent"
                ) {
                    return Err(unsupported("configuration has dependent or unknown rules"));
                }
            }
        }
        if target.is_some_and(|t| t.index.is_none()) {
            if let Some(value) = d.get("Intent") {
                let value = self.ctx.resolve(value)?;
                if !value.as_name().is_some_and(|n| n.name() == "View") {
                    return Err(unsupported("default Intent must be View"));
                }
            }
        }
        d.remove("Name");
        for (key, v) in &patch {
            d.try_insert(key.clone(), v.clone())?;
        }
        self.edit_transaction_with_budget(budget, |doc| {
            match target {
                Some(t) if t.index.is_none() => {
                    props.try_insert("D".into(), PdfObject::PdfDict(d))?;
                }
                Some(t) => {
                    let i = t.index.unwrap();
                    configs.remove(i);
                    configs.insert(i, PdfObject::PdfDict(d))?;
                    props.try_insert("Configs".into(), PdfObject::PdfArray(configs))?;
                }
                None => {
                    if configs.len() >= MAX_CONFIGS.min(doc.ctx.limits().max_objects) {
                        return Err(PdfError::ResourceLimit("layer configurations".into()));
                    }
                    configs.try_add_obj(PdfObject::PdfDict(d))?;
                    props.try_insert("Configs".into(), PdfObject::PdfArray(configs))?;
                }
            }
            let _retain = &old_lease;
            doc.store_layer_properties(root, props)
        })
    }
    fn layer_expression_object(
        &self,
        e: &LayerExpression,
        depth: usize,
        nodes: &mut usize,
    ) -> PdfResult<PdfObject> {
        *nodes = nodes
            .checked_add(1)
            .ok_or_else(|| invalid("expression overflow"))?;
        if *nodes > MAX_NODES.min(self.ctx.limits().max_content_operations)
            || depth >= 64.min(self.ctx.limits().max_depth)
        {
            return Err(PdfError::ResourceLimit("layer expression".into()));
        }
        match e {
            LayerExpression::Group(h) => {
                self.registered_layer(h)?;
                Ok(reference(h))
            }
            LayerExpression::Not(child) => array([
                name("Not"),
                self.layer_expression_object(child, depth + 1, nodes)?,
            ]),
            LayerExpression::And(children) | LayerExpression::Or(children) => {
                if children.len() < 2 || children.len() > MAX_NODES {
                    return Err(invalid("expression arity"));
                }
                let mut a = PdfArray::default();
                a.try_add_obj(name(if matches!(e, LayerExpression::And(_)) {
                    "And"
                } else {
                    "Or"
                }))?;
                for child in children {
                    a.try_add_obj(self.layer_expression_object(child, depth + 1, nodes)?)?;
                }
                Ok(PdfObject::PdfArray(a))
            }
        }
    }
    pub fn create_layer_membership(
        &self,
        groups: &[ObjectHandle],
        policy: LayerPolicy,
        expression: Option<&LayerExpression>,
        budget: &OperationBudget,
    ) -> PdfResult<ObjectHandle> {
        self.layer_guard(expression.is_some(), budget)?;
        let _scratch = self.layer_scratch(budget, 0)?;
        if groups.is_empty() || groups.len() > MAX_GROUPS.min(self.ctx.limits().max_objects) {
            return Err(invalid("membership group count"));
        }
        let mut seen = HashSet::new();
        for h in groups {
            self.registered_layer(h)?;
            if !seen.insert(h.object_id()) {
                return Err(invalid("duplicate membership group"));
            }
        }
        let mut d = PdfDict::default();
        d.try_insert("Type".into(), name("OCMD"))?;
        d.try_insert("OCGs".into(), array(groups.iter().map(reference))?)?;
        d.try_insert(
            "P".into(),
            name(match policy {
                LayerPolicy::AnyOn => "AnyOn",
                LayerPolicy::AllOn => "AllOn",
                LayerPolicy::AnyOff => "AnyOff",
                LayerPolicy::AllOff => "AllOff",
            }),
        )?;
        if let Some(e) = expression {
            if matches!(e, LayerExpression::Group(_)) {
                return Err(invalid("VE must be an expression"));
            }
            d.try_insert("VE".into(), self.layer_expression_object(e, 0, &mut 0)?)?;
        }
        self.edit_transaction_with_budget(budget, |doc| {
            let id = doc.try_insert_object(PdfObject::PdfDict(d))?;
            doc.object_handle(id)
        })
    }
}
