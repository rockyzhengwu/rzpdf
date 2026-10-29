//! Bounded metadata transport, password opening and stable error categories.
use super::*;
use pdf::{
    document::{destination::Destination, links::LinkTarget, outlines::OutlineTarget},
    error::PdfError,
    geom::matrix::Matrix,
    security::Authentication,
};
use serde_json::{Value, json};
use std::io::Write;
use zeroize::Zeroizing;

const REQUEST_LIMIT: usize = 64 * 1024;
const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
pub(crate) type ApiResult<T> = Result<T, ApiError>;
#[derive(Debug)]
pub(crate) struct ApiError {
    pub code: u32,
    pub message: String,
    pub details: Option<Value>,
}
impl ApiError {
    pub(crate) fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: (code == 7).then(|| json!({"reason":"unclassified","stage":"unknown"})),
        }
    }
    pub fn handle(message: &str) -> Self {
        Self::new(8, message)
    }
    pub fn resource(message: &str) -> Self {
        Self::new(7, message)
    }
}
impl From<&str> for ApiError {
    fn from(message: &str) -> Self {
        Self::new(9, message)
    }
}
impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self::new(9, message)
    }
}
impl From<PdfError> for ApiError {
    fn from(error: PdfError) -> Self {
        let code = match &error {
            PdfError::PasswordRequired => 2,
            PdfError::InvalidPassword => 3,
            PdfError::PermissionDenied(_) => 4,
            PdfError::UnsupportedFeature(_) | PdfError::UnsupportedSecurity(_) => 5,
            PdfError::FontError(_) => 6,
            PdfError::ResourceLimit(_) | PdfError::ResourceLimitExceeded { .. } => 7,
            _ => 1,
        };
        let mut result = Self::new(code, error.to_string());
        if let PdfError::ResourceLimitExceeded {
            reason,
            stage,
            limit,
            actual,
        } = error
        {
            result.details =
                Some(json!({"reason":reason,"stage":stage,"limit":limit,"actual":actual}));
        }
        result
    }
}
pub(crate) fn open(
    s: &mut State,
    input: u32,
    password: Option<&str>,
    mode: ParseMode,
) -> ApiResult<u32> {
    open_with_policy(s, input, password, mode, limits())
}
fn open_with_policy(
    s: &mut State,
    input: u32,
    password: Option<&str>,
    mode: ParseMode,
    policy: Limits,
) -> ApiResult<u32> {
    let Some(Resource::Input(bytes)) = s.resources.get(&input) else {
        return Err(ApiError::handle("invalid PDF input handle"));
    };
    if bytes.is_empty() {
        return Err(ApiError::resource("PDF input must not be empty"));
    }
    if s.resources.len() >= 8 || s.retained().saturating_add(bytes.len()) > BUFFER_LIMIT {
        return Err(ApiError::resource(
            "release buffers before opening a document",
        ));
    }
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ApiError::resource("PDF input allocation"))?;
    owned.extend_from_slice(bytes);
    let source_bytes = owned.len();
    let options = ParseOptions {
        mode,
        limits: policy,
    };
    let doc = match password {
        Some(password) => Document::from_bytes_with_password_and_options(owned, password, options),
        None => Document::from_bytes_with_options(owned, options),
    }?;
    s.insert(Resource::Document(Rc::new(doc), source_bytes))
}
pub(crate) fn request(s: &State, input: u32) -> ApiResult<Value> {
    let Some(Resource::Input(bytes)) = s.resources.get(&input) else {
        return Err(ApiError::handle("invalid request input handle"));
    };
    if bytes.len() > REQUEST_LIMIT {
        return Err(ApiError::resource("request exceeds 64 KiB"));
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| ApiError::from("invalid JSON request"))?;
    if !value.is_object() {
        return Err("request must be an object".into());
    }
    Ok(value)
}
struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > OUTPUT_LIMIT)
        {
            return Err(std::io::Error::other("JSON output limit"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| std::io::Error::other("JSON allocation"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn output(s: &mut State, value: Value) -> ApiResult<u32> {
    if s.resources.len() >= 8 {
        return Err(ApiError::resource("handle limit reached"));
    }
    let mut bytes = BoundedOutput(Vec::new());
    serde_json::to_writer(&mut bytes, &value)
        .map_err(|_| ApiError::resource("JSON output exceeds budget or allocation failed"))?;
    s.insert(Resource::Output(bytes.0))
}
fn matrix(m: &Matrix) -> [f32; 6] {
    [m.a(), m.b(), m.c(), m.d(), m.e(), m.f()]
}
fn page(request: &Value) -> ApiResult<u32> {
    request
        .get("page")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("invalid page index".into())
}
fn destination(d: &Document, target: &Destination) -> ApiResult<Value> {
    use pdf::document::destination::DestinationView::*;
    let view = match &target.view {
        Xyz { left, top, zoom } => json!({"kind":"XYZ","left":left,"top":top,"zoom":zoom}),
        Fit => json!({"kind":"Fit"}),
        FitB => json!({"kind":"FitB"}),
        FitH(top) => json!({"kind":"FitH","top":top}),
        FitBH(top) => json!({"kind":"FitBH","top":top}),
        FitV(left) => json!({"kind":"FitV","left":left}),
        FitBV(left) => json!({"kind":"FitBV","left":left}),
        FitR {
            left,
            bottom,
            right,
            top,
        } => json!({"kind":"FitR","rect":[left,bottom,right,top]}),
    };
    Ok(json!({"kind":"page","page":d.destination_page_index(target)?,"view":view}))
}
fn named_destination(d: &Document, name: &str) -> ApiResult<Value> {
    let target = d
        .named_destination_views()?
        .into_iter()
        .find(|(key, _)| key == name)
        .ok_or_else(|| ApiError::new(1, "named destination is missing"))?;
    destination(d, &target.1)
}
fn diagnostic(d: &pdf::page::diagnostic::ContentDiagnostic) -> Value {
    let source = d.source.as_ref().map(|s| json!({
        "operationIndex":s.operation_index,"byteOffset":s.operator_byte_offset,
        "invocations":s.invocations.iter().map(|v| json!({"object":v.form_id.map(|id|[id.obj_num(),id.gen_num()]),"operationIndex":v.operation_index,"byteOffset":v.operator_byte_offset})).collect::<Vec<_>>()
    }));
    json!({"kind":format!("{:?}",d.kind),"operator":d.operator,"byteOffset":d.byte_offset,"message":d.message,"source":source})
}
fn text(d: &Document, index: u32) -> ApiResult<Value> {
    if d.security_report()
        .is_some_and(|r| r.authentication == Authentication::User && !r.permissions.copy)
    {
        return Err(PdfError::PermissionDenied("text extraction".into()).into());
    }
    let report = d.extract_characters_for_display(index)?;
    let mut bytes = 0usize;
    let mut characters = Vec::new();
    for c in report.characters {
        // Bound the intermediate JSON tree, not just the final serializer.
        bytes = bytes.saturating_add(
            512 + c.unicode.as_ref().map_or(0, String::len)
                + c.font_name.as_ref().map_or(0, String::len),
        );
        if bytes > OUTPUT_LIMIT {
            return Err(ApiError::resource("text metadata exceeds output budget"));
        }
        let em = c.em_to_display_matrix().ok().as_ref().map(matrix);
        characters.push(
            json!({"unicode":c.unicode,"charcode":c.charcode,"codeLength":c.code_length,
            "position":[c.position.x(),c.position.y()],"advance":c.advance,"emToDisplay":em,"fontSize":c.font_size,
            "fontName":c.font_name,"writingMode":c.writing_mode.map(|w|format!("{w:?}")),
            "objectIndex":c.object_index,"operationIndex":c.source.map(|s|s.operation_index),
            "nested":c.nested_source.is_some()}),
        );
    }
    Ok(
        json!({"order":"paint","coordinateSpace":"display-points-top-left","characters":characters,
        "diagnostics":report.diagnostics.iter().map(diagnostic).collect::<Vec<_>>() }),
    )
}
fn query(d: &Document, document: u32, request: &Value) -> ApiResult<Value> {
    match request.get("operation").and_then(Value::as_str).ok_or("operation is required")? {
        "capabilities" => {
            let mut value = crate::resources::capabilities(d.resource_limits());
            value.as_object_mut().unwrap().extend(json!({"abi":2,"text":true,"navigation":true,"passwordOpen":true,
            "annotations":true,"forms":true,"editing":true,"save":true,"renderRegion":true,"maxRegionSide":1024,"maxRasterSide":32768,"regionRasterStatus":"experimental",
            "textOrder":"paint","maxInputBytes":null,"maxBufferBytes":BUFFER_LIMIT,"maxJsonBytes":OUTPUT_LIMIT,
            "revision":d.revision()}).as_object().unwrap().clone());
            Ok(value)
        },
        "cacheStatistics" => Ok(crate::resources::cache_statistics(d)),
        "clearSourceStreamCache" => {
            d.clear_source_stream_cache();
            Ok(crate::resources::cache_statistics(d))
        },
        "clearRenderCaches" => {
            d.clear_render_caches();
            #[cfg(target_os = "emscripten")]
            render::purge_isolated_web_caches();
            Ok(crate::resources::cache_statistics(d))
        },
        "state" | "annotations" | "formFields" | "content" => crate::interaction::query(d,document,request),
        "pageInfo" => {
            let g = d.get_page(page(request)?)?.geometry()?;
            Ok(json!({"width":g.display_width,"height":g.display_height,"rotation":g.rotation,"userUnit":g.user_unit,
                "visibleBox":[g.visible_box.llx(),g.visible_box.lly(),g.visible_box.urx(),g.visible_box.ury()],
                "userToDisplay":matrix(&g.user_to_display),"coordinateSpace":"display-points-top-left"}))
        }
        "text" => text(d, page(request)?),
        "outlines" => {
            let mut items = Vec::new();
            for item in d.outline_items()? {
                let target = match item.target {
                    OutlineTarget::Destination(target) => destination(d,&target)?,
                    OutlineTarget::Named(name) => named_destination(d,&name)?,
                };
                items.push(json!({"id":[item.handle.object_id().obj_num(),item.handle.object_id().gen_num()],
                    "parent":item.parent.map(|h|[h.object_id().obj_num(),h.object_id().gen_num()]),
                    "title":item.title,"open":item.open,"target":target}));
            }
            Ok(json!(items))
        }
        "links" => {
            let mut items = Vec::new();
            for item in d.page_links(page(request)?)? {
                let target = match item.target {
                    LinkTarget::Page(page) => json!({"kind":"page","page":page,"view":{"kind":"Fit"}}),
                    LinkTarget::Destination(target) => destination(d,&target)?,
                    LinkTarget::Named(name) => named_destination(d,&name)?,
                    LinkTarget::Uri(uri) => json!({"kind":"uri","uri":uri}),
                };
                items.push(json!({"rect":item.rect,"coordinateSpace":"pdf-user","target":target}));
            }
            Ok(json!(items))
        }
        "diagnostics" => Ok(json!(d.structure_diagnostics()?.into_iter().map(|v| json!({"reason":format!("{:?}",v.reason),"offset":v.offset,"object":v.object.map(|id|[id.obj_num(),id.gen_num()])})).collect::<Vec<_>>())),
        _ => Err(ApiError::new(5,"unsupported query operation")),
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_error_code() -> u32 {
    STATE.with(|s| s.borrow().error_code)
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_open_with_options(input: u32, options: u32) -> u32 {
    call(|s| {
        let Value::Object(mut options) = request(s, options)? else {
            unreachable!()
        };
        let mode = match options.remove("mode") {
            None => ParseMode::Strict,
            Some(Value::String(mode)) if mode == "strict" => ParseMode::Strict,
            Some(Value::String(mode)) if mode == "compatible" => ParseMode::Compatible,
            _ => return Err("mode must be strict or compatible".into()),
        };
        let policy = match options.remove("resourceProfile") {
            None => limits(),
            Some(Value::String(name)) => crate::resources::profile(&name)?,
            _ => return Err("resourceProfile must be a supported profile name".into()),
        };
        let password = match options.remove("password") {
            None => None,
            Some(Value::String(password)) if password.len() <= 1024 => {
                Some(Zeroizing::new(password))
            }
            _ => return Err("password must be a UTF-8 string of at most 1024 bytes".into()),
        };
        if !options.is_empty() {
            return Err("unknown open option".into());
        }
        open_with_policy(
            s,
            input,
            password.as_deref().map(String::as_str),
            mode,
            policy,
        )
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_query(document: u32, input: u32) -> u32 {
    call(|s| {
        let request = request(s, input)?;
        if document == 0 && request.get("operation").and_then(Value::as_str) == Some("capabilities")
        {
            return output(s, crate::resources::capabilities(limits()));
        }
        let Some(Resource::Document(d, _)) = s.resources.get(&document) else {
            return Err(ApiError::handle("invalid document handle"));
        };
        let operation = request.get("operation").and_then(Value::as_str);
        if operation == Some("clearRenderCaches")
            && let Some(session) = s.render_sessions.get(&document)
        {
            session.clear();
        }
        let mut value = if operation == Some("fontRequirements") {
            crate::fonts::requirements(s, document, &request)?
        } else {
            query(d, document, &request)?
        };
        if matches!(
            operation,
            Some("cacheStatistics" | "clearRenderCaches" | "clearSourceStreamCache")
        ) {
            let statistics = s
                .render_sessions
                .get(&document)
                .map(|session| session.statistics())
                .unwrap_or_else(|| {
                    render::render_device::RenderSession::new(d.resource_limits()).statistics()
                });
            value.as_object_mut().unwrap().extend(
                json!({
                    "renderSessionEstimatedBytes": statistics.estimated_bytes,
                    "renderSessionByteLimit": statistics.byte_limit,
                    "renderSessionFontEntries": statistics.font_entries,
                    "renderSessionFontManagerEntries": statistics.font_manager_entries,
                    "renderSessionFontManagerHits": statistics.font_manager_hits,
                    "renderSessionImageEntries": statistics.image_entries,
                    "renderSessionGeometryEntries": statistics.geometry_entries,
                    "renderSessionFontHits": statistics.font_hits,
                    "renderSessionImageHits": statistics.image_hits,
                    "renderSessionGeometryHits": statistics.geometry_hits,
                    "renderSessionMisses": statistics.misses,
                    "renderSessionEvictions": statistics.evictions
                })
                .as_object()
                .unwrap()
                .clone(),
            );
        }
        output(s, value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf::document::{destination::DestinationView, resource_manager::StandardFont};
    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }
    fn input(bytes: &[u8]) -> u32 {
        STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Input(bytes.to_vec()))
                .unwrap()
        })
    }
    fn options(value: Value) -> u32 {
        input(&serde_json::to_vec(&value).unwrap())
    }
    fn result(handle: u32) -> Value {
        assert_ne!(handle, 0);
        let value = STATE.with(|s| {
            serde_json::from_slice(s.borrow().resources[&handle].bytes().unwrap()).unwrap()
        });
        assert_eq!(super::super::rz_release(handle), 1);
        value
    }
    fn fixture() -> Vec<u8> {
        let d = Document::new().unwrap();
        d.add_page(0).unwrap();
        let font = d
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        d.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf 20 40 Td (Web text) Tj ET").into_bytes(),
        )
        .unwrap();
        let target = d.destination(0, DestinationView::Fit).unwrap();
        let parent = d
            .add_outline(None, "Chapter", &OutlineTarget::Destination(target.clone()))
            .unwrap();
        d.add_outline(Some(&parent), "Child", &OutlineTarget::Destination(target))
            .unwrap();
        d.add_uri_link(0, [20., 20., 80., 40.], "https://example.org/")
            .unwrap();
        d.save_to_bytes().unwrap()
    }
    fn read(document: u32, value: Value) -> u32 {
        let request = options(value);
        let output = rz_query(document, request);
        // Release only after copying errors in failure tests.
        STATE.with(|s| {
            s.borrow_mut().resources.remove(&request);
        });
        output
    }
    #[test]
    fn metadata_text_navigation_and_output_ownership() {
        reset();
        let bytes = fixture();
        let source = input(&bytes);
        let d = super::super::rz_open(source);
        assert_ne!(d, 0);
        assert_eq!(super::super::rz_release(source), 1);
        assert_eq!(STATE.with(|s| s.borrow().retained()), bytes.len());
        let info = result(read(d, json!({"operation":"pageInfo","page":0})));
        assert_eq!(info["coordinateSpace"], "display-points-top-left");
        assert!(info["width"].as_f64().unwrap() > 0.);
        let text = result(read(d, json!({"operation":"text","page":0})));
        let chars = text["characters"].as_array().unwrap();
        assert_eq!(
            chars
                .iter()
                .map(|c| c["unicode"].as_str().unwrap())
                .collect::<String>(),
            "Web text"
        );
        assert!(
            chars
                .iter()
                .all(|c| c["emToDisplay"].as_array().unwrap().len() == 6)
        );
        let tree = result(read(d, json!({"operation":"outlines"})));
        assert_eq!(tree[1]["parent"], tree[0]["id"]);
        assert_eq!(tree[1]["target"]["page"], 0);
        let links = result(read(d, json!({"operation":"links","page":0})));
        assert_eq!(links[0]["target"]["uri"], "https://example.org/");
        let output = read(d, json!({"operation":"pageInfo","page":0}));
        assert_eq!(super::super::rz_open(output), 0);
        assert_eq!(rz_error_code(), 8);
        assert_eq!(rz_query(d, output), 0);
        assert_eq!(rz_error_code(), 8);
        super::super::rz_release(output);
        super::super::rz_release(d);
        assert_eq!(STATE.with(|s| s.borrow().retained()), 0);
    }
    #[test]
    fn password_errors_are_distinct_retryable_and_do_not_leak_passwords() {
        reset();
        let source = input(include_bytes!(
            "../../pdf/tests/fixtures/r4/password-user.pdf"
        ));
        assert_eq!(super::super::rz_open(source), 0);
        assert_eq!(rz_error_code(), 2);
        let wrong = options(json!({"password":"never-print-this-secret"}));
        assert_eq!(rz_open_with_options(source, wrong), 0);
        assert_eq!(rz_error_code(), 3);
        assert!(!STATE.with(|s| s.borrow().error.contains("never-print")));
        super::super::rz_release(wrong);
        let good = options(json!({"password":"synthetic-user","mode":"compatible"}));
        let d = rz_open_with_options(source, good);
        assert_ne!(d, 0);
        assert_eq!(super::super::rz_page_count(d), 1);
        for h in [source, good, d] {
            assert_eq!(super::super::rz_release(h), 1);
        }
        assert!(STATE.with(|s| s.borrow().resources.is_empty()));
    }
    #[test]
    fn text_copy_permission_is_enforced_without_blocking_page_info() {
        reset();
        let doc = Document::from_bytes_with_mode(fixture(), ParseMode::Strict).unwrap();
        let mut settings = pdf::security::EncryptionOptions::new("user", "owner").unwrap();
        settings.permissions.copy = false;
        let bytes = doc
            .save_to_bytes_with_security(
                Default::default(),
                pdf::security::SaveSecurity::Set(&settings),
            )
            .unwrap();
        let source = input(&bytes);
        let opts = options(json!({"password":"user"}));
        let document = rz_open_with_options(source, opts);
        assert_ne!(document, 0);
        assert_eq!(read(document, json!({"operation":"text","page":0})), 0);
        assert_eq!(rz_error_code(), 4);
        let info = result(read(document, json!({"operation":"pageInfo","page":0})));
        assert!(info["width"].as_f64().unwrap() > 0.);
        for h in [source, opts, document] {
            super::super::rz_release(h);
        }
        assert_eq!(STATE.with(|s| s.borrow().retained()), 0);
    }
    #[test]
    fn request_validation_limits_and_stale_handles() {
        reset();
        let source = input(&fixture());
        for value in [
            json!({"mode":"guess"}),
            json!({"password":23}),
            json!({"mode":"strict","unexpected":true}),
        ] {
            let opts = options(value);
            assert_eq!(rz_open_with_options(source, opts), 0);
            assert_eq!(rz_error_code(), 9);
            assert_eq!(STATE.with(|s| s.borrow().resources.len()), 2);
            super::super::rz_release(opts);
        }
        let d = super::super::rz_open(source);
        assert_ne!(d, 0);
        assert_eq!(read(d, json!({"operation":"pageInfo","page":-1})), 0);
        assert_eq!(rz_error_code(), 9);
        assert_eq!(read(d, json!({"operation":"pageInfo","page":10})), 0);
        assert_eq!(rz_error_code(), 1);
        assert_eq!(read(d, json!({"operation":"not-supported"})), 0);
        assert_eq!(rz_error_code(), 5);
        let request = input(&vec![b' '; REQUEST_LIMIT + 1]);
        assert_eq!(rz_query(d, request), 0);
        assert_eq!(rz_error_code(), 7);
        super::super::rz_release(request);
        let mut writer = BoundedOutput(vec![0; OUTPUT_LIMIT]);
        assert!(writer.write_all(b"x").is_err());
        super::super::rz_release(d);
        assert_eq!(read(d, json!({"operation":"capabilities"})), 0);
        assert_eq!(rz_error_code(), 8);
        super::super::rz_release(source);
        assert!(STATE.with(|s| s.borrow().resources.is_empty()));
    }
}
