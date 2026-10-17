//! Bounded, read-only optional-content configurations and visibility snapshots.
//! No UI actions are executed and no document objects are changed.
use super::{
    PdfDocument,
    identity::{DocumentStamp, ObjectHandle},
    save_options::PdfVersion,
};
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict},
    pdf_context::PDFContext,
};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    io::{Read, Seek},
    rc::Rc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalContentIntent {
    View,
    Print,
}

#[derive(Debug, Clone)]
pub struct OptionalContentOptions {
    /// None selects /D; Some(index) selects the /Configs array by stable order.
    pub configuration: Option<usize>,
    pub intent: OptionalContentIntent,
    /// Viewer usage rules. False previews stand-alone printing/aggregation.
    pub apply_usage: bool,
    /// Explicit user choices applied last, without changing the source PDF.
    pub overrides: Vec<(ObjectHandle, bool)>,
}
impl Default for OptionalContentOptions {
    fn default() -> Self {
        Self {
            configuration: None,
            intent: OptionalContentIntent::View,
            apply_usage: true,
            overrides: vec![],
        }
    }
}
#[derive(Debug, Clone)]
pub struct OptionalContentGroup {
    pub handle: ObjectHandle,
    pub name: String,
    /// Unknown UI/usage metadata is retained, not interpreted as a visibility rule.
    pub dictionary: PdfDict,
}
#[derive(Debug, Clone)]
pub struct OptionalContentConfiguration {
    pub name: Option<String>,
    pub dictionary: PdfDict,
}
#[derive(Debug, Clone)]
pub struct OptionalContent {
    pub stamp: DocumentStamp,
    pub groups: Vec<OptionalContentGroup>,
    pub default_configuration: OptionalContentConfiguration,
    pub configurations: Vec<OptionalContentConfiguration>,
}
/// Immutable states tied to one document revision. Clones share cumulative work
/// accounting; they do not reset the expression-evaluation budget.
#[derive(Debug, Clone)]
pub struct OptionalContentVisibility {
    stamp: DocumentStamp,
    states: HashMap<ObjectId, bool>,
    considered: HashSet<ObjectId>,
    work: Rc<Cell<usize>>,
    has_properties: bool,
    version: PdfVersion,
    selection: OptionalContentSelection,
}

/// Cache discriminator, separate from source/edit provenance. Cloning a
/// visibility snapshot retains it; independently evaluating states creates a
/// new discriminator even if the chosen settings happen to be equivalent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptionalContentSelection {
    identity: super::identity::DocumentIdentity,
}

fn invalid(message: &str) -> PdfError {
    PdfError::DocumentError(format!("optional content: {message}"))
}
fn unsupported(message: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("optional content: {message}"))
}
fn effective_version<R: Read + Seek>(ctx: &PDFContext<R>) -> PdfResult<PdfVersion> {
    let header = PdfVersion::parse(ctx.source_version())
        .ok_or_else(|| unsupported("unknown PDF version"))?;
    let root = ctx.get_root_owned()?;
    match root.get("Version") {
        Some(value) => PdfDocument::<R>::merge_catalog_version(header, &ctx.resolve(value)?),
        None => Ok(header),
    }
}
fn charge<R: Read + Seek>(ctx: &PDFContext<R>, work: &Cell<usize>, amount: usize) -> PdfResult<()> {
    let value = work
        .get()
        .checked_add(amount)
        .filter(|v| *v <= ctx.limits().max_content_operations)
        .ok_or_else(|| PdfError::ResourceLimit("optional content cumulative work".into()))?;
    work.set(value);
    Ok(())
}
fn dict<R: Read + Seek>(ctx: &PDFContext<R>, value: &PdfObject) -> PdfResult<PdfDict> {
    ctx.resolve(value)?
        .into_dict()
        .ok_or_else(|| invalid("dictionary expected"))
}
fn array<R: Read + Seek>(ctx: &PDFContext<R>, value: &PdfObject) -> PdfResult<Vec<PdfObject>> {
    let value = ctx.resolve(value)?;
    Ok(value
        .as_array()
        .ok_or_else(|| invalid("array expected"))?
        .into_iter()
        .cloned()
        .collect())
}
fn name<R: Read + Seek>(ctx: &PDFContext<R>, value: &PdfObject) -> PdfResult<String> {
    Ok(ctx
        .resolve(value)?
        .as_name()
        .ok_or_else(|| invalid("name expected"))?
        .name()
        .to_owned())
}
fn text<R: Read + Seek>(ctx: &PDFContext<R>, value: &PdfObject) -> PdfResult<String> {
    ctx.resolve(value)?
        .as_string()
        .ok_or_else(|| invalid("text string expected"))?
        .text_content_with_limits(&ctx.limits())
}
fn intents<R: Read + Seek>(
    ctx: &PDFContext<R>,
    d: &PdfDict,
    work: &Cell<usize>,
) -> PdfResult<Vec<String>> {
    let Some(value) = d.get("Intent") else {
        return Ok(vec!["View".into()]);
    };
    let value = ctx.resolve(value)?;
    let values = if value.as_name().is_some() {
        vec![value]
    } else {
        array(ctx, &value)?
    };
    charge(ctx, work, values.len())?;
    values.iter().map(|v| name(ctx, v)).collect()
}
fn configuration<R: Read + Seek>(
    ctx: &PDFContext<R>,
    value: &PdfObject,
) -> PdfResult<OptionalContentConfiguration> {
    let d = dict(ctx, value)?;
    let label = d.get("Name").map(|v| text(ctx, v)).transpose()?;
    Ok(OptionalContentConfiguration {
        name: label,
        dictionary: d,
    })
}
impl<R: Read + Seek> PdfDocument<R> {
    pub fn optional_content(&self) -> PdfResult<Option<OptionalContent>> {
        OptionalContent::read(self.ctx())
    }
    /// Read and evaluate one configuration. A missing /OCProperties yields an
    /// all-visible snapshot, as required by PDF 1.7 section 8.11.4.2.
    pub fn optional_content_visibility(
        &self,
        options: &OptionalContentOptions,
    ) -> PdfResult<OptionalContentVisibility> {
        OptionalContentVisibility::from_context(self.ctx(), options)
    }
}
impl OptionalContent {
    fn read<R: Read + Seek>(ctx: &PDFContext<R>) -> PdfResult<Option<Self>> {
        let root = ctx.get_root_owned()?;
        let Some(properties) = root
            .get("OCProperties")
            .filter(|v| !matches!(v, PdfObject::PdfNull))
        else {
            return Ok(None);
        };
        let properties = dict(ctx, properties)?;
        if effective_version(ctx)? < PdfVersion::V1_5 {
            return Err(unsupported("OCProperties requires PDF 1.5 or later"));
        }
        let values = array(
            ctx,
            properties
                .get("OCGs")
                .ok_or_else(|| invalid("OCGs missing"))?,
        )?;
        let work = Cell::new(0);
        charge(ctx, &work, values.len())?;
        if values.len() > ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("optional content groups".into()));
        }
        let mut seen = HashSet::new();
        let mut groups = Vec::new();
        let mut bytes = 0usize;
        let mut chars = 0usize;
        for value in values {
            let id = value
                .as_object_id()
                .ok_or_else(|| invalid("OCGs must contain indirect groups"))?;
            if !seen.insert(id) {
                return Err(invalid("duplicate OCG"));
            }
            let d = dict(ctx, &value)?;
            if d.get("Type").map(|v| name(ctx, v)).transpose()?.as_deref() != Some("OCG") {
                return Err(invalid("OCG Type"));
            }
            let label = text(
                ctx,
                d.get("Name").ok_or_else(|| invalid("OCG Name missing"))?,
            )?;
            bytes = bytes
                .checked_add(label.len())
                .filter(|n| *n <= ctx.limits().max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("optional content names".into()))?;
            chars = chars
                .checked_add(label.chars().count())
                .filter(|n| *n <= ctx.limits().max_text_chars)
                .ok_or_else(|| {
                    PdfError::ResourceLimit("optional content name characters".into())
                })?;
            groups.push(OptionalContentGroup {
                handle: ObjectHandle {
                    document: ctx.identity(),
                    id,
                },
                name: label,
                dictionary: d,
            });
        }
        let default_configuration = configuration(
            ctx,
            properties
                .get("D")
                .ok_or_else(|| invalid("default configuration missing"))?,
        )?;
        let mut configurations = Vec::new();
        if let Some(values) = properties.get("Configs") {
            let values = array(ctx, values)?;
            charge(ctx, &work, values.len())?;
            if groups.len().saturating_add(values.len()) > ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit(
                    "optional content configurations".into(),
                ));
            }
            for value in values {
                configurations.push(configuration(ctx, &value)?);
            }
        }
        for c in std::iter::once(&default_configuration).chain(&configurations) {
            chars = chars
                .checked_add(c.name.as_ref().map_or(0, |v| v.chars().count()))
                .filter(|n| *n <= ctx.limits().max_text_chars)
                .ok_or_else(|| {
                    PdfError::ResourceLimit("optional content cumulative name characters".into())
                })?;
            bytes = bytes
                .checked_add(c.name.as_ref().map_or(0, String::len))
                .filter(|n| *n <= ctx.limits().max_decoded_bytes)
                .ok_or_else(|| {
                    PdfError::ResourceLimit("optional content cumulative names".into())
                })?;
        }
        Ok(Some(Self {
            stamp: DocumentStamp {
                document: ctx.identity(),
                revision: ctx.revision(),
            },
            groups,
            default_configuration,
            configurations,
        }))
    }
}
impl OptionalContentVisibility {
    pub(crate) fn from_context<R: Read + Seek>(
        ctx: &PDFContext<R>,
        options: &OptionalContentOptions,
    ) -> PdfResult<Self> {
        let mut result = Self {
            stamp: DocumentStamp {
                document: ctx.identity(),
                revision: ctx.revision(),
            },
            states: HashMap::new(),
            considered: HashSet::new(),
            work: Rc::new(Cell::new(0)),
            has_properties: false,
            version: effective_version(ctx)?,
            selection: OptionalContentSelection {
                identity: super::identity::DocumentIdentity::new(),
            },
        };
        let Some(model) = OptionalContent::read(ctx)? else {
            if options.configuration.is_some() || !options.overrides.is_empty() {
                return Err(invalid("selection without OCProperties"));
            }
            return Ok(result);
        };
        result.has_properties = true;
        charge(
            ctx,
            &result.work,
            model
                .groups
                .len()
                .saturating_add(model.configurations.len()),
        )?;
        for group in &model.groups {
            result.states.insert(group.handle.id, true);
        }
        result.apply_config(ctx, &model.default_configuration.dictionary)?;
        let selected = if let Some(index) = options.configuration {
            let config = model
                .configurations
                .get(index)
                .ok_or_else(|| invalid("configuration index"))?;
            result.apply_config(ctx, &config.dictionary)?;
            config
        } else {
            &model.default_configuration
        };
        let selected_intents = intents(ctx, &selected.dictionary, &result.work)?;
        for group in &model.groups {
            let group_intents = intents(ctx, &group.dictionary, &result.work)?;
            if selected_intents
                .iter()
                .any(|v| v == "All" || group_intents.contains(v))
            {
                result.considered.insert(group.handle.id);
            }
        }
        if options.apply_usage {
            result.apply_usage(ctx, &model, &selected.dictionary, "View")?;
            if options.intent == OptionalContentIntent::Print {
                result.apply_usage(ctx, &model, &selected.dictionary, "Print")?;
            }
        }
        let mut seen = HashSet::new();
        for (handle, state) in &options.overrides {
            charge(ctx, &result.work, 1)?;
            if handle.document != result.stamp.document || !seen.insert(handle.id) {
                return Err(invalid("foreign or duplicate override"));
            }
            *result
                .states
                .get_mut(&handle.id)
                .ok_or_else(|| invalid("override group not registered"))? = *state;
        }
        Ok(result)
    }
    fn group_ids<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        value: &PdfObject,
    ) -> PdfResult<Vec<ObjectId>> {
        let values = array(ctx, value)?;
        charge(ctx, &self.work, values.len())?;
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        for value in values {
            let id = value
                .as_object_id()
                .filter(|id| self.states.contains_key(id))
                .ok_or_else(|| invalid("unregistered OCG reference"))?;
            if !seen.insert(id) {
                return Err(invalid("duplicate group in configuration array"));
            }
            ids.push(id);
        }
        Ok(ids)
    }
    fn apply_config<R: Read + Seek>(&mut self, ctx: &PDFContext<R>, d: &PdfDict) -> PdfResult<()> {
        charge(ctx, &self.work, 1)?;
        for (key, _) in d {
            if !matches!(
                key.as_str(),
                "Name"
                    | "Creator"
                    | "BaseState"
                    | "ON"
                    | "OFF"
                    | "Intent"
                    | "AS"
                    | "Order"
                    | "ListMode"
                    | "RBGroups"
                    | "Locked"
            ) {
                return Err(unsupported("unknown configuration extension"));
            }
        }
        let base = d
            .get("BaseState")
            .map(|v| name(ctx, v))
            .transpose()?
            .unwrap_or_else(|| "ON".into());
        match base.as_str() {
            "ON" | "OFF" => {
                charge(ctx, &self.work, self.states.len())?;
                for state in self.states.values_mut() {
                    *state = base == "ON";
                }
            }
            "Unchanged" => {}
            _ => return Err(unsupported("BaseState")),
        }
        let on = d
            .get("ON")
            .map(|v| self.group_ids(ctx, v))
            .transpose()?
            .unwrap_or_default();
        let off = d
            .get("OFF")
            .map(|v| self.group_ids(ctx, v))
            .transpose()?
            .unwrap_or_default();
        let on_set: HashSet<_> = on.iter().copied().collect();
        if off.iter().any(|id| on_set.contains(id)) {
            return Err(invalid("group in both ON and OFF"));
        }
        for id in on {
            self.states.insert(id, true);
        }
        for id in off {
            self.states.insert(id, false);
        }
        Ok(())
    }
    fn apply_usage<R: Read + Seek>(
        &mut self,
        ctx: &PDFContext<R>,
        model: &OptionalContent,
        d: &PdfDict,
        event: &str,
    ) -> PdfResult<()> {
        let Some(value) = d.get("AS") else {
            return Ok(());
        };
        let applications = array(ctx, value)?;
        charge(ctx, &self.work, applications.len())?;
        let groups: HashMap<_, _> = model
            .groups
            .iter()
            .map(|g| (g.handle.id, &g.dictionary))
            .collect();
        let mut recommendations = HashMap::new();
        for value in applications {
            let application = dict(ctx, &value)?;
            let e = name(
                ctx,
                application
                    .get("Event")
                    .ok_or_else(|| invalid("AS Event missing"))?,
            )?;
            if !matches!(e.as_str(), "View" | "Print" | "Export") {
                return Err(unsupported("AS Event"));
            }
            let categories = array(
                ctx,
                application
                    .get("Category")
                    .ok_or_else(|| invalid("AS Category missing"))?,
            )?;
            charge(ctx, &self.work, categories.len())?;
            let ids = application
                .get("OCGs")
                .map(|v| self.group_ids(ctx, v))
                .transpose()?
                .unwrap_or_default();
            if e != event {
                continue;
            }
            for category in &categories {
                if !matches!(name(ctx, category)?.as_str(), "View" | "Print" | "Export") {
                    return Err(unsupported(
                        "AS Category requires an unsupported environment",
                    ));
                }
            }
            for id in ids {
                let Some(usage) = groups[&id].get("Usage") else {
                    continue;
                };
                let usage = dict(ctx, usage)?;
                for category in &categories {
                    charge(ctx, &self.work, 1)?;
                    let category = name(ctx, category)?;
                    // No environment-dependent guesses for language, user,
                    // zoom, or future categories, even if the group omits them.
                    let key = match category.as_str() {
                        "View" => "ViewState",
                        "Print" => "PrintState",
                        "Export" => "ExportState",
                        _ => {
                            return Err(unsupported(
                                "AS Category requires an unsupported environment",
                            ));
                        }
                    };
                    let Some(value) = usage.get(&category) else {
                        continue;
                    };
                    let usage = dict(ctx, value)?;
                    let Some(value) = usage.get(key) else {
                        continue;
                    };
                    let state = match name(ctx, value)?.as_str() {
                        "ON" => true,
                        "OFF" => false,
                        _ => return Err(invalid("usage state")),
                    };
                    let entry = recommendations.entry(id).or_insert(true);
                    *entry &= state;
                }
            }
        }
        for (id, state) in recommendations {
            self.states.insert(id, state);
        }
        Ok(())
    }
    pub fn stamp(&self) -> &DocumentStamp {
        &self.stamp
    }
    pub fn selection(&self) -> &OptionalContentSelection {
        &self.selection
    }
    pub fn group_state(&self, handle: &ObjectHandle) -> PdfResult<bool> {
        if handle.document != self.stamp.document {
            return Err(invalid("foreign group handle"));
        }
        self.states
            .get(&handle.id)
            .copied()
            .ok_or_else(|| invalid("unregistered group"))
    }
    pub(crate) fn validate<R: Read + Seek>(&self, ctx: &PDFContext<R>) -> PdfResult<()> {
        if self.stamp.document != ctx.identity() || self.stamp.revision != ctx.revision() {
            return Err(invalid("stale or foreign visibility snapshot"));
        }
        Ok(())
    }
    pub fn is_visible<R: Read + Seek>(
        &self,
        document: &PdfDocument<R>,
        membership: &PdfObject,
    ) -> PdfResult<bool> {
        self.visible(document.ctx(), membership)
    }
    pub(crate) fn visible<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        membership: &PdfObject,
    ) -> PdfResult<bool> {
        self.validate(ctx)?;
        // No OCProperties: ignore optional content structures, not their paints.
        if !self.has_properties {
            return Ok(true);
        }
        self.evaluate(ctx, membership, 0, &mut HashSet::new(), false)
    }
    fn evaluate<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        value: &PdfObject,
        depth: usize,
        active: &mut HashSet<ObjectId>,
        expression: bool,
    ) -> PdfResult<bool> {
        charge(ctx, &self.work, 1)?;
        if depth > ctx.limits().max_depth.min(128) {
            return Err(PdfError::ResourceLimit(
                "optional content expression depth".into(),
            ));
        }
        let id = value.as_object_id();
        if let Some(id) = id {
            if let Some(state) = self.states.get(&id) {
                if expression && !self.considered.contains(&id) {
                    return Err(unsupported(
                        "nonmatching group intent in OCMD/VE requires explicit reconciliation",
                    ));
                }
                return Ok(!self.considered.contains(&id) || *state);
            }
            if !active.insert(id) {
                return Err(invalid("cyclic visibility expression"));
            }
        }
        let value = ctx.resolve(value)?;
        let result = if let Some(values) = value.as_array() {
            if !expression {
                return Err(invalid("membership must be OCG or OCMD"));
            }
            let operator = values.get(0).ok_or_else(|| invalid("empty VE"))?;
            let operator = name(ctx, operator)?;
            if !matches!(operator.as_str(), "And" | "Or" | "Not") {
                return Err(unsupported("VE operator"));
            }
            if values.len() < 2 || (operator == "Not" && values.len() != 2) {
                return Err(invalid("VE arity"));
            }
            let mut result = operator == "And";
            for child in values.into_iter().skip(1) {
                // Deliberately no short circuit: invalid/cyclic hidden branches
                // must not become accepted because of another group's state.
                let state = self.evaluate(ctx, child, depth + 1, active, true)?;
                match operator.as_str() {
                    "And" => result &= state,
                    "Or" => result |= state,
                    "Not" => result = !state,
                    _ => unreachable!(),
                }
            }
            result
        } else {
            let d = value
                .as_dict()
                .ok_or_else(|| invalid("membership dictionary expected"))?;
            match d.get("Type").map(|v| name(ctx, v)).transpose()?.as_deref() {
                Some("OCG") => return Err(invalid("OCG is not registered by indirect identity")),
                Some("OCMD") if !expression => {
                    for (key, _) in d {
                        if !matches!(key.as_str(), "Type" | "OCGs" | "P" | "VE") {
                            return Err(unsupported("unknown membership extension"));
                        }
                    }
                    if let Some(ve) = d.get("VE") {
                        if self.version < PdfVersion::V1_6 {
                            return Err(unsupported("VE requires PDF 1.6 or later"));
                        }
                        self.evaluate(ctx, ve, depth + 1, active, true)?
                    } else {
                        let policy = d
                            .get("P")
                            .map(|v| name(ctx, v))
                            .transpose()?
                            .unwrap_or_else(|| "AnyOn".into());
                        if !matches!(policy.as_str(), "AnyOn" | "AllOn" | "AnyOff" | "AllOff") {
                            return Err(unsupported("membership policy"));
                        }
                        let mut states = Vec::new();
                        if let Some(groups) = d.get("OCGs") {
                            let resolved = ctx.resolve(groups)?;
                            let values = if resolved.as_array().is_some() {
                                array(ctx, &resolved)?
                            } else {
                                vec![groups.clone()]
                            };
                            charge(ctx, &self.work, values.len())?;
                            for group in values {
                                if matches!(ctx.resolve(&group)?, PdfObject::PdfNull) {
                                    continue;
                                }
                                if !group
                                    .as_object_id()
                                    .is_some_and(|id| self.states.contains_key(&id))
                                {
                                    return Err(invalid("OCMD requires registered OCGs"));
                                }
                                states.push(self.evaluate(ctx, &group, depth + 1, active, true)?);
                            }
                        }
                        if states.is_empty() {
                            true
                        } else {
                            match policy.as_str() {
                                "AnyOn" => states.iter().any(|s| *s),
                                "AllOn" => states.iter().all(|s| *s),
                                "AnyOff" => states.iter().any(|s| !s),
                                "AllOff" => states.iter().all(|s| !s),
                                _ => unreachable!(),
                            }
                        }
                    }
                }
                _ => return Err(invalid("expected OCG leaf or OCMD membership")),
            }
        };
        if let Some(id) = id {
            active.remove(&id);
        }
        Ok(result)
    }
}
