//! WA-WEB-04: typed interaction, revision-bound atomic mutations and bounded saves.
use super::*;
use crate::api::request;
use pdf::{
    document::{
        acroform::{FormFieldKind, FormValue},
        annotations::{Annotation, AnnotationKind as Kind, AnnotationRemoval, TextAlignment},
        save_options::SaveOptions,
    },
    error::{PdfError, PdfResult},
    objects::object_id::ObjectId,
    security::Authentication,
};
use serde_json::{Value, json};
const REPLY_LIMIT: usize = 512;
fn object(id: ObjectId) -> [u32; 2] {
    [id.obj_num(), id.gen_num()]
}
fn stamp(d: &Document, document: u32) -> Value {
    json!({"document":document,"revision":d.revision().to_string()})
}
fn permissions(d: &Document) -> Value {
    let report = d.security_report();
    let allowed = |f: fn(pdf::security::Permissions) -> bool| {
        report.is_none_or(|r| {
            r.revision == 6 && (r.authentication == Authentication::Owner || f(r.permissions))
        })
    };
    json!({"annotate":allowed(|p|p.annotate),"fillForms":allowed(|p|p.fill_forms),"modify":allowed(|p|p.modify),"assemble":allowed(|p|p.assemble),
        "copy":report.is_none_or(|r|r.authentication==Authentication::Owner||r.permissions.copy),"save":report.is_none_or(|r|r.revision==6),"securityRevision":report.map(|r|r.revision)})
}
fn kind(k: &Kind) -> Value {
    match k {
        Kind::Square => json!({"kind":"Square"}),
        Kind::Circle => json!({"kind":"Circle"}),
        Kind::Text { icon, open } => json!({"kind":"Text","icon":icon,"open":open}),
        Kind::Popup { open } => json!({"kind":"Popup","open":open}),
        Kind::FreeText {
            font_size,
            alignment,
        } => {
            json!({"kind":"FreeText","fontSize":font_size,"alignment":match alignment {TextAlignment::Left=>"left",TextAlignment::Center=>"center",TextAlignment::Right=>"right"}})
        }
        Kind::Line { points } => json!({"kind":"Line","points":points}),
        Kind::Polygon { vertices } => json!({"kind":"Polygon","vertices":vertices}),
        Kind::PolyLine { vertices } => json!({"kind":"PolyLine","vertices":vertices}),
        Kind::Highlight { quads } => json!({"kind":"Highlight","quads":quads}),
        Kind::Underline { quads } => json!({"kind":"Underline","quads":quads}),
        Kind::Squiggly { quads } => json!({"kind":"Squiggly","quads":quads}),
        Kind::StrikeOut { quads } => json!({"kind":"StrikeOut","quads":quads}),
        Kind::Ink { strokes } => json!({"kind":"Ink","strokes":strokes}),
        Kind::Stamp { name } => json!({"kind":"Stamp","name":name}),
        Kind::FileAttachment { file, icon } => {
            json!({"kind":"FileAttachment","file":object(file.object_id()),"icon":icon})
        }
    }
}
fn annotation(a: &Annotation) -> Value {
    let mut value = kind(&a.kind);
    let fields = value.as_object_mut().unwrap();
    fields.insert("rect".into(), json!(a.rect));
    fields.insert("color".into(), json!(a.color));
    fields.insert("interiorColor".into(), json!(a.interior_color));
    fields.insert("contents".into(), json!(a.contents));
    fields.insert("flags".into(), json!(a.flags));
    fields.insert("opacity".into(), json!(a.opacity));
    fields.insert("strokeWidth".into(), json!(a.stroke_width));
    value
}
fn value(v: &FormValue) -> Value {
    match v {
        FormValue::Text(text) => json!({"kind":"text","text":text}),
        FormValue::Checkbox(checked) => json!({"kind":"checkbox","checked":checked}),
        FormValue::ButtonState(state) => json!({"kind":"buttonState","state":state}),
        FormValue::Radio(state) => json!({"kind":"radio","state":state}),
        FormValue::Choice(values) => json!({"kind":"choice","values":values}),
        FormValue::ChoiceIndices(indices) => json!({"kind":"choiceIndices","indices":indices}),
        FormValue::PushButton => json!({"kind":"pushButton"}),
    }
}
pub(crate) fn query(d: &Document, document: u32, request: &Value) -> ApiResult<Value> {
    match request["operation"].as_str().unwrap_or("") {
        "state" => Ok(
            json!({"stamp":stamp(d,document),"commitRevision":d.commit_revision().to_string(),"pageCount":d.try_page_count()?,"permissions":permissions(d)}),
        ),
        "content" => crate::content::query(d, document, uint(&request["page"])?),
        "annotations" => {
            let page = uint(&request["page"])?;
            let records = d.page_annotations(page)?;
            Ok(
                json!({"stamp":stamp(d,document),"page":page,"records":records.into_iter().map(|r|json!({"object":r.handle.map(|h|object(h.object_id())),"subtype":r.subtype,"annotation":r.annotation.as_ref().map(annotation),"unsupportedReason":r.unsupported_reason,
                "popup":r.popup.map(|h|object(h.object_id())),"popupParent":r.popup_parent.map(|h|object(h.object_id())),"replyTo":r.reply_to.map(|h|object(h.object_id()))})).collect::<Vec<_>>()}),
            )
        }
        "formFields" => {
            let mut records = Vec::new();
            let visibility = d.optional_content_visibility(&Default::default())?;
            let mut bytes = 0usize;
            for field in d.form_field_records()? {
                bytes = bytes.saturating_add(512 + field.name.len() + field.widgets.len() * 128);
                if bytes > 8 * 1024 * 1024 {
                    return Err(ApiError::resource("form metadata budget"));
                }
                let detail=field.details.as_ref().map(|f| {
                    let k=match &f.kind {FormFieldKind::Text{multiline}=>json!({"kind":"text","multiline":multiline}),FormFieldKind::Checkbox=>json!({"kind":"checkbox"}),FormFieldKind::Radio{no_toggle_to_off,in_unison}=>json!({"kind":"radio","noToggleToOff":no_toggle_to_off,"inUnison":in_unison}),FormFieldKind::Choice{combo,editable,multiple}=>json!({"kind":"choice","combo":combo,"editable":editable,"multiple":multiple}),FormFieldKind::PushButton=>json!({"kind":"pushButton"})};
                    json!({"type":k,"value":value(&f.value),"options":f.options.iter().map(|o|json!({"export":o.export,"display":o.display})).collect::<Vec<_>>(),"buttonStates":f.button_states,"buttonExports":f.button_exports})
                });
                records.push(json!({"object":object(field.handle.object_id()),"name":field.name,"fieldType":field.field_type,"flags":field.flags,
                    "widgets":field.widgets.iter().enumerate().map(|(i,w)| -> ApiResult<Value> {
                        let dict=d.resolve_handle(&w.handle)?.into_dict().ok_or_else(||ApiError::new(1,"widget dictionary expected"))?;
                        let mut flag_value=dict.get("F").cloned();
                        for _ in 0..32 {
                            if let Some(reference)=flag_value.as_ref().and_then(|v|v.as_reference()) {
                                flag_value=Some(d.resolve_object(reference.id())?);
                            } else {break;}
                        }
                        let flags=flag_value.as_ref().map(|v|v.as_u32().ok_or_else(||ApiError::new(1,"widget flags integer expected"))).transpose()?.unwrap_or(0);
                        let visible=flags & (2|32)==0 && match dict.get("OC") {
                            Some(oc)=>visibility.is_visible(d,oc)?,None=>true
                        };
                        Ok(json!({"object":object(w.handle.object_id()),"page":w.page_index,"rect":w.rect,"flags":flags,"visible":visible,"onState":field.details.as_ref().and_then(|f|f.button_states.get(i))}))
                    }).collect::<ApiResult<Vec<_>>>()?,
                    "details":detail,"unsupportedReason":field.unsupported_reason}));
            }
            Ok(json!({"stamp":stamp(d,document),"records":records}))
        }
        _ => Err(ApiError::new(5, "unsupported interaction query")),
    }
}
fn keys(v: &Value, allowed: &[&str]) -> ApiResult<()> {
    if !v
        .as_object()
        .is_some_and(|o| o.keys().all(|k| allowed.contains(&k.as_str())))
    {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> ApiError {
    ApiError::new(9, "invalid interaction request")
}
fn uint(v: &Value) -> ApiResult<u32> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(invalid)
}
fn number(v: &Value) -> ApiResult<f32> {
    v.as_f64()
        .map(|n| n as f32)
        .filter(|n| n.is_finite())
        .ok_or_else(invalid)
}
fn text(v: &Value) -> ApiResult<String> {
    v.as_str()
        .filter(|s| s.len() <= 16 * 1024)
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn array<const N: usize>(v: &Value) -> ApiResult<[f32; N]> {
    let a = v.as_array().filter(|a| a.len() == N).ok_or_else(invalid)?;
    let mut out = [0.; N];
    for (i, n) in a.iter().enumerate() {
        out[i] = number(n)?;
    }
    Ok(out)
}
fn arrays<const N: usize>(v: &Value) -> ApiResult<Vec<[f32; N]>> {
    let a = v
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or_else(invalid)?;
    a.iter().map(array).collect()
}
fn id(v: &Value) -> ApiResult<ObjectId> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(invalid)?;
    let n = uint(&a[0])?;
    let g = uint(&a[1])?;
    if n == 0 || g > 65535 {
        return Err(invalid());
    }
    Ok(ObjectId::new(n, g))
}
fn scalar(v: &Value, key: &str, default: f32) -> ApiResult<f32> {
    v.get(key).map(number).unwrap_or(Ok(default))
}
fn boolean(v: &Value, key: &str, default: bool) -> ApiResult<bool> {
    v.get(key)
        .map(|v| v.as_bool().ok_or_else(invalid))
        .unwrap_or(Ok(default))
}
fn optional_text(v: &Value, key: &str, default: &str) -> ApiResult<String> {
    v.get(key).map(text).unwrap_or_else(|| Ok(default.into()))
}
fn parse_annotation(d: &Document, v: &Value) -> ApiResult<Annotation> {
    keys(
        v,
        &[
            "kind",
            "rect",
            "color",
            "interiorColor",
            "contents",
            "flags",
            "opacity",
            "strokeWidth",
            "icon",
            "open",
            "fontSize",
            "alignment",
            "points",
            "vertices",
            "quads",
            "strokes",
            "name",
            "file",
        ],
    )?;

    let tag = v["kind"].as_str().ok_or_else(invalid)?;
    let kind = match tag {
        "Square" => Kind::Square,
        "Circle" => Kind::Circle,
        "Text" => Kind::Text {
            icon: optional_text(v, "icon", "Note")?,
            open: boolean(v, "open", false)?,
        },
        "Popup" => Kind::Popup {
            open: boolean(v, "open", false)?,
        },
        "FreeText" => Kind::FreeText {
            font_size: scalar(v, "fontSize", 12.)?,
            alignment: match v["alignment"].as_str().unwrap_or("left") {
                "left" => TextAlignment::Left,
                "center" => TextAlignment::Center,
                "right" => TextAlignment::Right,
                _ => return Err(invalid()),
            },
        },
        "Line" => Kind::Line {
            points: array(&v["points"])?,
        },
        "Polygon" => Kind::Polygon {
            vertices: arrays(&v["vertices"])?,
        },
        "PolyLine" => Kind::PolyLine {
            vertices: arrays(&v["vertices"])?,
        },
        "Highlight" => Kind::Highlight {
            quads: arrays(&v["quads"])?,
        },
        "Underline" => Kind::Underline {
            quads: arrays(&v["quads"])?,
        },
        "Squiggly" => Kind::Squiggly {
            quads: arrays(&v["quads"])?,
        },
        "StrikeOut" => Kind::StrikeOut {
            quads: arrays(&v["quads"])?,
        },
        "Ink" => {
            let strokes = v["strokes"]
                .as_array()
                .filter(|a| a.len() <= 4096)
                .ok_or_else(invalid)?
                .iter()
                .map(arrays)
                .collect::<ApiResult<Vec<Vec<[f32; 2]>>>>()?;
            if strokes.iter().map(Vec::len).sum::<usize>() > 4096 {
                return Err(ApiError::resource("ink point limit"));
            }
            Kind::Ink { strokes }
        }
        "Stamp" => Kind::Stamp {
            name: optional_text(v, "name", "Draft")?,
        },
        "FileAttachment" => Kind::FileAttachment {
            file: d.object_handle(id(&v["file"])?)?,
            icon: optional_text(v, "icon", "PushPin")?,
        },
        _ => return Err(ApiError::new(5, "unsupported annotation kind")),
    };
    let mut a = Annotation::new(kind, array(&v["rect"])?);
    a.color = v.get("color").map(array).unwrap_or(Ok([1., 0., 0.]))?;
    a.interior_color = v
        .get("interiorColor")
        .filter(|v| !v.is_null())
        .map(array)
        .transpose()?;
    a.contents = optional_text(v, "contents", "")?;
    a.flags = v.get("flags").map(uint).unwrap_or(Ok(4))?;
    a.opacity = scalar(v, "opacity", 1.)?;
    a.stroke_width = scalar(v, "strokeWidth", 1.)?;
    Ok(a)
}
fn parse_value(v: &Value) -> ApiResult<FormValue> {
    keys(
        v,
        &["kind", "text", "checked", "state", "values", "indices"],
    )?;

    let state = || {
        v.get("state").ok_or_else(invalid).and_then(|v| {
            if v.is_null() {
                Ok(None)
            } else {
                text(v).map(Some)
            }
        })
    };
    match v["kind"].as_str().ok_or_else(invalid)? {
        "text" => Ok(FormValue::Text(text(&v["text"])?)),
        "checkbox" => Ok(FormValue::Checkbox(
            v["checked"].as_bool().ok_or_else(invalid)?,
        )),
        "radio" => Ok(FormValue::Radio(state()?)),
        "buttonState" => Ok(FormValue::ButtonState(state()?)),
        "choice" => Ok(FormValue::Choice(
            v["values"]
                .as_array()
                .filter(|a| a.len() <= 4096)
                .ok_or_else(invalid)?
                .iter()
                .map(text)
                .collect::<ApiResult<_>>()?,
        )),
        "choiceIndices" => Ok(FormValue::ChoiceIndices(
            v["indices"]
                .as_array()
                .filter(|a| a.len() <= 4096)
                .ok_or_else(invalid)?
                .iter()
                .map(uint)
                .collect::<ApiResult<_>>()?,
        )),
        _ => Err(ApiError::new(
            5,
            "unsupported form value or executable action",
        )),
    }
}
enum Command {
    Add(u32, Annotation),
    Update(u32, ObjectId, Annotation),
    Remove(u32, ObjectId, bool),
    Field(ObjectId, FormValue),
    Content(crate::content::Command),
    Import(
        Rc<Document>,
        Vec<u32>,
        u32,
        pdf::document::page_transfer::PageTransferOptions,
    ),
}
fn prepare(d: &Document, r: &Value) -> ApiResult<Command> {
    if matches!(
        r["operation"].as_str(),
        Some(
            "replaceText"
                | "replaceTextSpan"
                | "removeImage"
                | "transformImage"
                | "replaceImage"
                | "rotatePage"
                | "copyPage"
                | "deletePage"
                | "reorderPages"
        )
    ) {
        return crate::content::prepare(r).map(Command::Content);
    }
    keys(
        r,
        &[
            "operation",
            "stamp",
            "page",
            "object",
            "annotation",
            "value",
            "cascade",
        ],
    )?;

    match r["operation"].as_str().ok_or_else(invalid)? {
        "addAnnotation" => Ok(Command::Add(
            uint(&r["page"])?,
            parse_annotation(d, &r["annotation"])?,
        )),
        "updateAnnotation" => Ok(Command::Update(
            uint(&r["page"])?,
            id(&r["object"])?,
            parse_annotation(d, &r["annotation"])?,
        )),
        "removeAnnotation" => Ok(Command::Remove(
            uint(&r["page"])?,
            id(&r["object"])?,
            boolean(r, "cascade", false)?,
        )),
        "setFormValue" => Ok(Command::Field(id(&r["object"])?, parse_value(&r["value"])?)),
        _ => Err(ApiError::new(5, "unsupported mutation")),
    }
}
fn apply(
    d: &Document,
    command: Command,
    asset: Option<&[u8]>,
    budget: &pdf::budget::OperationBudget,
) -> PdfResult<Option<ObjectId>> {
    match command {
        Command::Import(source, pages, insertion, options) => {
            d.import_pages_from_with_options(&source, &pages, insertion, &options)?;
            Ok(None)
        }
        Command::Content(command) => crate::content::apply(d, command, asset, budget),
        Command::Add(p, a) => Ok(Some(d.add_annotation(p, &a)?.object_id())),
        Command::Update(p, id, a) => {
            d.update_annotation(p, &d.object_handle(id)?, &a)?;
            Ok(None)
        }
        Command::Remove(p, id, cascade) => {
            d.remove_annotation(
                p,
                &d.object_handle(id)?,
                if cascade {
                    AnnotationRemoval::Cascade
                } else {
                    AnnotationRemoval::RejectRelated
                },
            )?;
            Ok(None)
        }
        Command::Field(id, value) => {
            let node = d
                .form_field_tree()?
                .into_iter()
                .find(|n| n.handle.object_id() == id)
                .ok_or_else(|| {
                    PdfError::InvalidReference("field is not in document tree".into())
                })?;
            d.set_form_value(&node.name, value)?;
            Ok(None)
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_apply(document: u32, input: u32) -> u32 {
    rz_apply_asset(document, input, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_apply_asset(document: u32, input: u32, asset: u32) -> u32 {
    call(|s| {
        let budget = limits().operation_budget();
        let r = request(s, input)?;
        let Some(Resource::Document(d, _)) = s.resources.get(&document) else {
            return Err(ApiError::handle("invalid document handle"));
        };
        let d = Rc::clone(d);
        if r["stamp"]["document"].as_u64() != Some(u64::from(document))
            || r["stamp"]["revision"].as_str() != Some(d.revision().to_string().as_str())
        {
            return Err(ApiError::handle("stale or foreign document revision"));
        }
        let command = if r["operation"].as_str() == Some("importPages") {
            keys(
                &r,
                &[
                    "operation",
                    "stamp",
                    "source",
                    "pages",
                    "insertion",
                    "namePrefix",
                    "includeNavigation",
                ],
            )?;
            let source_handle = uint(&r["source"]["document"])?;
            if source_handle == document {
                return Err(ApiError::new(9, "use copyPage for the current document"));
            }
            let Some(Resource::Document(source, _)) = s.resources.get(&source_handle) else {
                return Err(ApiError::handle("source document is closed or invalid"));
            };
            if r["source"]["revision"].as_str() != Some(source.revision().to_string().as_str()) {
                return Err(ApiError::handle("stale source document revision"));
            }
            if source.security_report().is_some_and(|report| {
                report.authentication == Authentication::User && !report.permissions.copy
            }) {
                return Err(ApiError::new(4, "source document copy permission denied"));
            }
            let pages = r["pages"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 10000)
                .ok_or_else(invalid)?
                .iter()
                .map(uint)
                .collect::<ApiResult<Vec<_>>>()?;
            Command::Import(
                Rc::clone(source),
                pages,
                uint(&r["insertion"])?,
                pdf::document::page_transfer::PageTransferOptions {
                    name_prefix: optional_text(&r, "namePrefix", "")?,
                    include_navigation: boolean(&r, "includeNavigation", false)?,
                    ..Default::default()
                },
            )
        } else {
            prepare(&d, &r)?
        };
        let asset_limit = match &command {
            Command::Content(c) => c.asset_limit(),
            _ => None,
        };
        let needs_asset = asset_limit.is_some();
        if needs_asset != (asset != 0) {
            return Err(ApiError::new(
                9,
                "mutation requires its declared binary asset",
            ));
        }
        let asset = if asset == 0 {
            None
        } else {
            let Some(Resource::Input(bytes)) = s.resources.get(&asset) else {
                return Err(ApiError::handle("invalid mutation asset input"));
            };
            if bytes.is_empty() || bytes.len() > asset_limit.unwrap() {
                return Err(ApiError::resource("mutation asset exceeds limit"));
            }
            Some(pdf::budget::BudgetedBytes::copy_from_slice(&budget, bytes)?)
        };
        if let Some(security) = d.security_report() {
            if security.revision != 6 {
                return Err(ApiError::new(5, "security handler is read-only in Web"));
            }
            let allowed = match command {
                Command::Field(..) => security.permissions.fill_forms,
                Command::Content(..) => security.permissions.modify,
                Command::Import(..) => security.permissions.modify && security.permissions.assemble,
                _ => security.permissions.annotate,
            };
            if security.authentication == Authentication::User && !allowed {
                return Err(ApiError::new(4, "document permission denied"));
            }
        }
        // Reserve the reply resource before entering a transaction. Its replacement
        // after commit is infallible; every fallible serialization remains inside.
        let mut reserve = Vec::new();
        reserve
            .try_reserve_exact(REPLY_LIMIT)
            .map_err(|_| ApiError::resource("mutation reply allocation"))?;
        reserve.resize(REPLY_LIMIT, 0);
        let reply = s.insert(Resource::Output(reserve))?;
        let result=d.edit_transaction_with_budget(&budget,|doc|{
            let created=apply(doc,command,asset.as_ref().map(|b|b.as_slice()),&budget)?;
            let value=json!({"stamp":stamp(doc,document),"pageCount":doc.try_page_count()?,"created":created.map(object)});
            let bytes=serde_json::to_vec(&value).map_err(|_|PdfError::ResourceLimit("mutation reply serialization".into()))?;
            if bytes.len()>REPLY_LIMIT{return Err(PdfError::ResourceLimit("mutation reply size".into()));}Ok(bytes)
        });
        match result {
            Ok(bytes) => {
                let Some(Resource::Output(buffer)) = s.resources.get_mut(&reply) else {
                    unreachable!()
                };
                *buffer = bytes;
                Ok(reply)
            }
            Err(error) => {
                s.resources.remove(&reply);
                Err(error.into())
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_save(document: u32, max_bytes: u32) -> u32 {
    call(|s| {
        if max_bytes == 0 || max_bytes as usize > SAVE_LIMIT {
            return Err(ApiError::resource(
                "save limit exceeds the shared buffer budget",
            ));
        }
        if s.resources.len() >= 8 {
            return Err(ApiError::resource("release buffers before saving"));
        }
        let available = BUFFER_LIMIT
            .saturating_sub(s.retained())
            .min(max_bytes as usize);
        if available == 0 {
            return Err(ApiError::resource("save buffer budget"));
        }
        let Some(Resource::Document(d, _)) = s.resources.get(&document) else {
            return Err(ApiError::handle("invalid document handle"));
        };
        let bytes = d.save_to_bytes_with_options(SaveOptions {
            max_output_bytes: available,
            ..Default::default()
        })?;
        s.insert(Resource::Output(bytes))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(bytes: &[u8]) -> u32 {
        STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Input(bytes.to_vec()))
                .unwrap()
        })
    }
    fn message(v: Value) -> u32 {
        input(&serde_json::to_vec(&v).unwrap())
    }
    fn bytes(h: u32) -> Vec<u8> {
        STATE.with(|s| s.borrow().resources[&h].bytes().unwrap().to_vec())
    }
    fn json(h: u32) -> Value {
        assert_ne!(h, 0, "{}", STATE.with(|s| s.borrow().error.clone()));
        let value = serde_json::from_slice(&bytes(h)).unwrap();
        rz_release(h);
        value
    }
    fn query(h: u32, operation: &str, page: Option<u32>) -> Value {
        let r = message(json!({"operation":operation,"page":page}));
        let result = crate::api::rz_query(h, r);
        assert_ne!(
            result,
            0,
            "query {operation}: {}",
            STATE.with(|s| s.borrow().error.clone())
        );
        rz_release(r);
        json(result)
    }
    fn mutate(h: u32, r: Value) -> u32 {
        let request = message(r);
        let result = rz_apply(h, request);
        STATE.with(|s| {
            s.borrow_mut().resources.remove(&request);
        });
        result
    }
    fn source() -> Vec<u8> {
        let d = Document::new().unwrap();
        d.add_page(0).unwrap();
        d.add_text_field(0, [10., 10., 180., 35.], "name", "Old")
            .unwrap();
        d.save_to_bytes().unwrap()
    }
    fn open(bytes: &[u8], password: Option<&str>) -> u32 {
        let i = input(bytes);
        let o = message(json!({"password":password.unwrap_or(""),"mode":"strict"}));
        let d = crate::api::rz_open_with_options(i, o);
        rz_release(i);
        rz_release(o);
        assert_ne!(d, 0);
        d
    }
    #[test]
    fn widget_screen_metadata_tracks_hidden_no_view_and_read_only_flags() {
        use pdf::objects::{PdfObject, pdf_number::PdfNumber};
        rz_dispose();
        let handle = open(&source(), None);
        let doc = STATE.with(
            |state| match state.borrow().resources.get(&handle).unwrap() {
                Resource::Document(d, _) => Rc::clone(d),
                _ => panic!("document expected"),
            },
        );
        let widget = doc.form_field_records().unwrap()[0].widgets[0]
            .handle
            .clone();
        for (flags, visible) in [
            (4, true),
            (2, false),
            (32, false),
            (64, true),
            (8 | 16, true),
        ] {
            let mut dict = doc.resolve_handle(&widget).unwrap().into_dict().unwrap();
            dict.try_insert(
                "F".into(),
                PdfObject::PdfNumber(PdfNumber::new(flags as f32)),
            )
            .unwrap();
            doc.update_object(widget.object_id(), PdfObject::PdfDict(dict))
                .unwrap();
            let fields = query(handle, "formFields", None);
            assert_eq!(fields["records"][0]["widgets"][0]["flags"], flags);
            assert_eq!(fields["records"][0]["widgets"][0]["visible"], visible);
        }
        use pdf::document::optional_content_edit::{
            LayerBaseState, LayerConfiguration, LayerIntent,
        };
        let budget = limits().operation_budget();
        let layer = doc
            .create_layer("Hidden widget", LayerIntent::View, &budget)
            .unwrap();
        let target = doc.layer_configuration_target(None).unwrap();
        doc.write_layer_configuration(
            Some(&target),
            &LayerConfiguration {
                name: None,
                base: LayerBaseState::On,
                on: vec![],
                off: vec![layer.clone()],
            },
            &budget,
        )
        .unwrap();
        let mut dict = doc.resolve_handle(&widget).unwrap().into_dict().unwrap();
        dict.try_insert(
            "OC".into(),
            PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(
                layer.object_id(),
            )),
        )
        .unwrap();
        doc.update_object(widget.object_id(), PdfObject::PdfDict(dict))
            .unwrap();
        assert_eq!(
            query(handle, "formFields", None)["records"][0]["widgets"][0]["visible"],
            false
        );
        let target = doc.layer_configuration_target(None).unwrap();
        doc.write_layer_configuration(
            Some(&target),
            &LayerConfiguration {
                name: None,
                base: LayerBaseState::On,
                on: vec![layer],
                off: vec![],
            },
            &budget,
        )
        .unwrap();
        assert_eq!(
            query(handle, "formFields", None)["records"][0]["widgets"][0]["visible"],
            true
        );
        rz_dispose();
    }
    #[test]
    fn content_edits_use_source_ranges_atomic_assets_and_page_copy_isolation() {
        use pdf::document::resource_manager::StandardFont;
        rz_dispose();
        let original = Document::from_bytes(source()).unwrap();
        let font = original
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let image = original
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        original.replace_page_contents(0,format!("BT /{font} 12 Tf 20 100 Td (Alpha) Tj (Tail) Tj ET q 20 0 0 20 20 30 cm /{image} Do Q").into_bytes()).unwrap();
        let d = open(&original.save_to_bytes().unwrap(), None);
        let before = query(d, "content", Some(0));
        let tail = before["characters"][5]["position"].clone();
        let result = json(mutate(
            d,
            json!({"operation":"replaceText","stamp":before["stamp"],"page":0,"start":0,"end":5,"text":"New"}),
        ));
        let after = query(d, "content", Some(0));
        assert_eq!(after["characters"][3]["position"], tail);
        let state = query(d, "state", None);
        assert_eq!(
            mutate(
                d,
                json!({"operation":"replaceText","stamp":result["stamp"],"page":0,"start":0,"end":3,"text":"中文"})
            ),
            0
        );
        assert_eq!(crate::api::rz_error_code(), 5);
        assert_eq!(
            query(d, "state", None)["commitRevision"],
            state["commitRevision"]
        );
        let current = query(d, "content", Some(0));
        let transform = json(mutate(
            d,
            json!({"operation":"transformImage","stamp":current["stamp"],"page":0,"index":0,"matrix":[1,0,0,1,10,15]}),
        ));
        let moved = query(d, "content", Some(0));
        assert_eq!(
            moved["images"][0]["matrix"][4].as_f64().unwrap(),
            current["images"][0]["matrix"][4].as_f64().unwrap() + 10.
        );
        assert_eq!(
            moved["images"][0]["matrix"][5].as_f64().unwrap(),
            current["images"][0]["matrix"][5].as_f64().unwrap() + 15.
        );
        let request = message(
            json!({"operation":"replaceImage","stamp":transform["stamp"],"page":0,"index":0,"format":"png"}),
        );
        let bad = input(b"bad png");
        assert_eq!(rz_apply_asset(d, request, bad), 0);
        assert_eq!(crate::api::rz_error_code(), 1);
        rz_release(bad);
        rz_release(request);
        let fresh = query(d, "state", None);
        let commit = fresh["commitRevision"].clone();
        let request = message(
            json!({"operation":"replaceImage","stamp":fresh["stamp"],"page":0,"index":0,"format":"png"}),
        );
        let asset = input(include_bytes!("../../pdf/tests/fixtures/v01/pixels.png"));
        let reply = rz_apply_asset(d, request, asset);
        assert_ne!(reply, 0);
        let replaced = json(reply);
        rz_release(request);
        rz_release(asset);
        assert_ne!(query(d, "state", None)["commitRevision"], commit);
        let copied = json(mutate(
            d,
            json!({"operation":"copyPage","stamp":replaced["stamp"],"page":0,"insertion":1,"namePrefix":"copy"}),
        ));
        assert_eq!(copied["pageCount"], 2);
        let selected = query(d, "content", Some(0));
        let changed = json(mutate(
            d,
            json!({"operation":"replaceText","stamp":selected["stamp"],"page":0,"start":0,"end":3,"text":"One"}),
        ));
        let copy = query(d, "content", Some(1));
        assert_eq!(copy["characters"][0]["unicode"], "N");
        let reordered = json(mutate(
            d,
            json!({"operation":"reorderPages","stamp":changed["stamp"],"order":[1,0]}),
        ));
        assert_eq!(
            query(d, "content", Some(0))["characters"][0]["unicode"],
            "N"
        );
        let removed = json(mutate(
            d,
            json!({"operation":"deletePage","stamp":reordered["stamp"],"page":1}),
        ));
        assert_eq!(removed["pageCount"], 1);
        assert_eq!(
            mutate(
                d,
                json!({"operation":"deletePage","stamp":removed["stamp"],"page":0})
            ),
            0
        );
        assert_eq!(crate::api::rz_error_code(), 5);
        let saved = rz_save(d, SAVE_LIMIT as u32);
        assert_ne!(saved, 0);
        let data = bytes(saved);
        rz_release(saved);
        let reopened = open(&data, None);
        assert_eq!(
            query(reopened, "content", Some(0))["characters"][0]["unicode"],
            "N"
        );
        rz_dispose();
    }
    #[test]
    fn imports_validate_both_revisions_preserve_source_and_survive_source_close() {
        rz_dispose();
        let target = open(&source(), None);
        let source_doc = open(&source(), None);
        let target_state = query(target, "state", None);
        let source_state = query(source_doc, "state", None);
        let before = rz_save(target, SAVE_LIMIT as u32);
        let before_bytes = bytes(before);
        rz_release(before);
        assert_eq!(
            mutate(
                target,
                json!({"operation":"importPages","stamp":target_state["stamp"],"source":source_state["stamp"],"pages":[0],"insertion":1})
            ),
            0
        );
        let failed = query(target, "state", None);
        assert_eq!(failed["commitRevision"], target_state["commitRevision"]);
        let after = rz_save(target, SAVE_LIMIT as u32);
        assert_eq!(bytes(after), before_bytes);
        rz_release(after);
        let result = json(mutate(
            target,
            json!({"operation":"importPages","stamp":failed["stamp"],"source":source_state["stamp"],"pages":[0],"insertion":1,"namePrefix":"from"}),
        ));
        assert_eq!(result["pageCount"], 2);
        assert_eq!(
            query(source_doc, "state", None)["stamp"],
            source_state["stamp"]
        );
        let source_change = json(mutate(
            source_doc,
            json!({"operation":"addAnnotation","stamp":source_state["stamp"],"page":0,"annotation":{"kind":"Square","rect":[20,20,60,60]}}),
        ));
        assert_eq!(
            mutate(
                target,
                json!({"operation":"importPages","stamp":result["stamp"],"source":source_state["stamp"],"pages":[0],"insertion":2,"namePrefix":"again"})
            ),
            0
        );
        assert_eq!(crate::api::rz_error_code(), 8);
        rz_release(source_doc);
        assert_eq!(
            mutate(
                target,
                json!({"operation":"importPages","stamp":result["stamp"],"source":source_change["stamp"],"pages":[0],"insertion":2,"namePrefix":"closed"})
            ),
            0
        );
        assert_eq!(crate::api::rz_error_code(), 8);
        let rendered = rz_render_page(target, 1, 72., 3, 1.);
        assert_ne!(rendered, 0);
        rz_release(rendered);
        let saved = rz_save(target, SAVE_LIMIT as u32);
        let output = bytes(saved);
        rz_release(saved);
        let reopened = open(&output, None);
        assert_eq!(query(reopened, "state", None)["pageCount"], 2);
        let fields = query(reopened, "formFields", None);
        assert!(
            fields["records"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["name"] == "from_name")
        );
        rz_dispose();
    }
    #[test]
    fn licensed_font_assets_write_new_subset_glyphs_atomically_and_isolate_shared_page() {
        use pdf::document::embedded_font::TrueTypeEmbeddingOptions;
        use pdf::page::content_builder::ContentBuilder;
        let full = include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf");
        rz_dispose();
        let original = Document::new().unwrap();
        original.add_page(0).unwrap();
        original.add_page(1).unwrap();
        let font = original
            .resources()
            .embed_truetype_font(
                0,
                full.to_vec(),
                "你",
                TrueTypeEmbeddingOptions {
                    postscript_name: "NotoSansSC-Thin".into(),
                    cap_height: 733.,
                    stem_v: 80.,
                    serif: false,
                },
            )
            .unwrap();
        for page in 0..2 {
            let name = font.register_on_page(&original, page).unwrap();
            let mut body = ContentBuilder::new();
            body.begin_text();
            body.try_set_font(&name, 14.).unwrap();
            body.move_text(20., 200.);
            body.show_encoded_text(&font.encoding().encode_text("你").unwrap())
                .unwrap();
            body.end_text();
            original.replace_page_content_builder(page, &body).unwrap();
        }
        let document = open(&original.save_to_bytes().unwrap(), None);
        let before = query(document, "content", Some(0));
        let metadata = json!({"postscriptName":"NotoSansSC-Thin","capHeight":733,"stemV":80,"serif":false,"omitLayoutAndAuxiliary":true});
        let request = message(
            json!({"operation":"replaceText","stamp":before["stamp"],"page":0,"start":0,"end":1,"text":"新","font":metadata}),
        );
        let asset = input(full);
        let reply = rz_apply_asset(document, request, asset);
        assert_ne!(reply, 0, "{}", STATE.with(|s| s.borrow().error.clone()));
        json(reply);
        rz_release(request);
        rz_release(asset);
        let changed = query(document, "content", Some(0));
        assert_eq!(changed["characters"][0]["unicode"], "新");
        assert!(
            changed["characters"][0]["fontName"]
                .as_str()
                .unwrap()
                .starts_with("AAAAAA+")
        );
        assert_eq!(
            query(document, "content", Some(1))["characters"][0]["unicode"],
            "你"
        );
        let saved = rz_save(document, SAVE_LIMIT as u32);
        let saved_bytes = bytes(saved);
        rz_release(saved);
        let request = message(
            json!({"operation":"replaceText","stamp":changed["stamp"],"page":0,"start":0,"end":1,"text":"你","font":metadata}),
        );
        let bad = input(b"invalid TTF");
        assert_eq!(rz_apply_asset(document, request, bad), 0);
        assert_eq!(crate::api::rz_error_code(), 6);
        rz_release(request);
        rz_release(bad);
        let unchanged = rz_save(document, SAVE_LIMIT as u32);
        assert_eq!(bytes(unchanged), saved_bytes);
        rz_release(unchanged);
        let reopened = open(&saved_bytes, None);
        assert_eq!(
            query(reopened, "content", Some(0))["characters"][0]["unicode"],
            "新"
        );
        rz_dispose();
    }
    #[test]
    fn field_annotation_save_reopen_and_stale_foreign_guards() {
        rz_dispose();
        let source = source();
        let d = open(&source, None);
        let other = open(&source, None);
        let fields = query(d, "formFields", None);
        let state = query(d, "state", None);
        assert_eq!(fields["records"][0]["details"]["value"]["text"], "Old");
        let set = json!({"operation":"setFormValue","stamp":fields["stamp"],"object":fields["records"][0]["object"],"value":{"kind":"text","text":"New"}});
        assert_eq!(mutate(other, set.clone()), 0);
        assert_eq!(crate::api::rz_error_code(), 8);
        let result = json(mutate(d, set.clone()));
        assert_ne!(result["stamp"], state["stamp"]);
        assert_eq!(query(d, "state", None)["stamp"], result["stamp"]);
        assert_eq!(mutate(d, set), 0);
        assert_eq!(crate::api::rz_error_code(), 8);
        let added = json(mutate(
            d,
            json!({"operation":"addAnnotation","stamp":result["stamp"],"page":0,"annotation":{"kind":"Square","rect":[20,50,80,90],"color":[1,0,0],"contents":"Note"}}),
        ));
        let annots = query(d, "annotations", Some(0));
        assert_eq!(
            annots["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["subtype"] == "Square")
                .unwrap()["object"],
            added["created"]
        );
        let image = rz_render_page(d, 0, 72., 3, 1.);
        assert_ne!(image, 0);
        rz_release(image);
        let without = rz_render(d, 0, 72.);
        assert_ne!(without, 0);
        rz_release(without);
        let saved = rz_save(d, SAVE_LIMIT as u32);
        assert_ne!(saved, 0);
        let saved_bytes = bytes(saved);
        assert_eq!(rz_open(saved), 0);
        assert_eq!(crate::api::rz_error_code(), 8);
        rz_release(saved);
        let reopened = open(&saved_bytes, None);
        assert_eq!(
            query(reopened, "formFields", None)["records"][0]["details"]["value"]["text"],
            "New"
        );
        let next = query(reopened, "annotations", Some(0));
        let square = next["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["subtype"] == "Square")
            .unwrap();
        assert_eq!(square["annotation"]["contents"], "Note");
        let update = json(mutate(
            reopened,
            json!({"operation":"updateAnnotation","stamp":next["stamp"],"page":0,"object":square["object"],"annotation":{"kind":"Square","rect":[20,50,90,100],"color":[0,0,1],"contents":"Updated"}}),
        ));
        json(mutate(
            reopened,
            json!({"operation":"removeAnnotation","stamp":update["stamp"],"page":0,"object":square["object"]}),
        ));
        assert_eq!(
            query(reopened, "annotations", Some(0))["records"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["annotation"].is_object())
                .count(),
            0
        );
        rz_dispose();
        assert_eq!(rz_handle_count(), 0);
        assert_eq!(rz_retained_bytes(), 0);
    }
    #[test]
    fn rollback_reply_capacity_and_save_failure_do_not_partially_edit() {
        rz_dispose();
        let d = open(&source(), None);
        let before = query(d, "state", None);
        let command = json!({"operation":"addAnnotation","stamp":before["stamp"],"page":0,"annotation":{"kind":"Square","rect":[20,50,80,90]}});
        let r = message(command.clone());
        let mut held = Vec::new();
        while rz_handle_count() < 8 {
            held.push(rz_alloc(1));
        }
        assert_eq!(rz_apply(d, r), 0);
        assert_eq!(crate::api::rz_error_code(), 7);
        for h in held {
            rz_release(h);
        }
        rz_release(r);
        assert_eq!(query(d, "state", None)["stamp"], before["stamp"]);
        let result = json(mutate(d, command));
        let bad = json!({"operation":"addAnnotation","stamp":result["stamp"],"page":0,"annotation":{"kind":"FreeText","rect":[20,100,80,130],"contents":"中文"}});
        let commit = query(d, "state", None)["commitRevision"].clone();
        assert_eq!(mutate(d, bad), 0);
        assert_eq!(crate::api::rz_error_code(), 5);
        let after = query(d, "state", None);
        assert_eq!(after["commitRevision"], commit);
        assert_ne!(after["stamp"], result["stamp"]);
        assert_eq!(
            query(d, "annotations", Some(0))["records"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["annotation"].is_object())
                .count(),
            1
        );
        assert_eq!(rz_save(d, 1), 0);
        assert_eq!(crate::api::rz_error_code(), 7);
        assert_eq!(query(d, "state", None)["stamp"], after["stamp"]);
        let saved = rz_save(d, SAVE_LIMIT as u32);
        assert_ne!(saved, 0);
        rz_release(saved);
        rz_dispose();
    }
    #[test]
    fn restricted_r6_can_fill_and_preserve_security_but_r4_remains_read_only() {
        rz_dispose();
        let doc = Document::from_bytes(source()).unwrap();
        let mut protection = pdf::security::EncryptionOptions::new("user", "owner").unwrap();
        protection.permissions.modify = false;
        protection.permissions.annotate = false;
        protection.permissions.fill_forms = true;
        let encrypted = doc
            .save_to_bytes_with_security(
                Default::default(),
                pdf::security::SaveSecurity::Set(&protection),
            )
            .unwrap();
        let d = open(&encrypted, Some("user"));
        let fields = query(d, "formFields", None);
        let result = json(mutate(
            d,
            json!({"operation":"setFormValue","stamp":fields["stamp"],"object":fields["records"][0]["object"],"value":{"kind":"text","text":"Allowed"}}),
        ));
        assert_eq!(
            mutate(
                d,
                json!({"operation":"addAnnotation","stamp":result["stamp"],"page":0,"annotation":{"kind":"Square","rect":[10,50,50,80]}})
            ),
            0
        );
        assert_eq!(crate::api::rz_error_code(), 4);
        assert_eq!(query(d, "state", None)["stamp"], result["stamp"]);
        let saved = rz_save(d, SAVE_LIMIT as u32);
        assert_ne!(saved, 0);
        let protected = bytes(saved);
        rz_release(saved);
        assert!(Document::from_bytes(protected.clone()).is_err());
        let reopened = Document::from_bytes_with_password(protected, "user").unwrap();
        assert_eq!(reopened.security_report().unwrap().revision, 6);
        assert!(!reopened.security_report().unwrap().permissions.annotate);
        let r4 = open(
            include_bytes!("../../pdf/tests/fixtures/r4/empty-user.pdf"),
            None,
        );
        let state = query(r4, "state", None);
        assert_eq!(state["permissions"]["securityRevision"], 4);
        assert_eq!(state["permissions"]["fillForms"], false);
        assert_eq!(
            mutate(
                r4,
                json!({"operation":"addAnnotation","stamp":state["stamp"],"page":0,"annotation":{"kind":"Square","rect":[10,10,30,30]}})
            ),
            0
        );
        assert_eq!(crate::api::rz_error_code(), 5);
        assert_eq!(rz_save(r4, SAVE_LIMIT as u32), 0);
        assert_eq!(crate::api::rz_error_code(), 5);
        rz_dispose();
    }
}
