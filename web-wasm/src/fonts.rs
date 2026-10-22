//! Explicit multi-font registration; asynchronous fetching remains in JS.
use super::*;
use pdf::font::resolver::FontMetadata;
use serde_json::{Value, json};
#[unsafe(no_mangle)]
pub extern "C" fn rz_register_font(input: u32, options: u32) -> u32 {
    call(|s| {
        let request = api::request(s, options)?;
        let object = request
            .as_object()
            .ok_or("font options must be an object")?;
        if object.keys().any(|k| {
            !matches!(
                k.as_str(),
                "family"
                    | "aliases"
                    | "weight"
                    | "italic"
                    | "serif"
                    | "languages"
                    | "index"
                    | "document"
                    | "revision"
                    | "fontEpoch"
            )
        }) {
            return Err("unknown font option".into());
        }
        match (object.get("document"), object.get("revision")) {
            (None, None) if !object.contains_key("fontEpoch") => {}
            (Some(document), Some(revision)) => {
                let document = document
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or("invalid font document")?;
                let Some(Resource::Document(d, _)) = s.resources.get(&document) else {
                    return Err(ApiError::handle("font document is closed"));
                };
                if object
                    .get("fontEpoch")
                    .is_some_and(|v| v.as_str() != Some(s.font_epoch.to_string().as_str()))
                {
                    return Err(ApiError::handle("stale font registry epoch"));
                }
                if revision.as_str() != Some(d.revision().to_string().as_str()) {
                    return Err(ApiError::handle("stale font document revision"));
                }
            }
            _ => return Err("font registration requires document and revision together".into()),
        }
        let string = |key: &str| -> ApiResult<String> {
            object
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("invalid font {key}").into())
        };
        let strings = |key: &str| -> ApiResult<Vec<String>> {
            match object.get(key) {
                None => Ok(vec![]),
                Some(Value::Array(v)) if v.len() <= 16 => v
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| "invalid font string list".into())
                    })
                    .collect(),
                _ => Err("invalid font string list".into()),
            }
        };
        let boolean = |key: &str| -> ApiResult<bool> {
            object
                .get(key)
                .and_then(Value::as_bool)
                .ok_or_else(|| format!("invalid font {key}").into())
        };
        let index = object
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or("invalid font index")?;
        let metadata = FontMetadata {
            family: string("family")?,
            aliases: strings("aliases")?,
            weight: object
                .get("weight")
                .and_then(Value::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .ok_or("invalid font weight")?,
            italic: boolean("italic")?,
            serif: boolean("serif")?,
            languages: strings("languages")?,
        };
        let Some(Resource::Input(bytes)) = s.resources.get(&input) else {
            return Err(ApiError::handle("invalid font input"));
        };
        let mut next = s.font_provider.as_ref().clone();
        let identity = next.register(bytes, index, metadata)?;
        if s.retained() - s.font_provider.byte_len() + next.byte_len() + 512 > BUFFER_LIMIT {
            return Err(ApiError::resource("retained font buffer limit"));
        }
        let sha256: String = identity.sha256.iter().map(|b| format!("{b:02x}")).collect();
        // Reserve/serialize the reply before changing the registry.
        let output = api::output(
            s,
            json!({"sha256":sha256,"index":identity.collection_index}),
        )?;
        s.font_provider = Rc::new(next);
        Ok(output)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_clear_fonts() -> u32 {
    call(|s| {
        let epoch = s
            .font_epoch
            .checked_add(1)
            .ok_or_else(|| ApiError::resource("font epoch exhausted"))?;
        s.font_provider = Default::default();
        s.font_epoch = epoch;
        Ok(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf::font::resolver::FontProvider;
    fn input(bytes: &[u8]) -> u32 {
        STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Input(bytes.to_vec()))
                .unwrap()
        })
    }
    fn options(family: &str) -> u32 {
        input(
            &serde_json::to_vec(
                &json!({"family":family,"weight":400,"italic":false,"serif":false,"index":0}),
            )
            .unwrap(),
        )
    }
    #[test]
    fn registry_identity_idempotence_failures_reply_reservation_and_dispose() {
        STATE.with(|s| *s.borrow_mut() = Default::default());
        let source = include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf");
        let font = input(source);
        let request = options("CJK");
        let out = rz_register_font(font, request);
        assert_ne!(out, 0);
        let first = STATE.with(|s| {
            serde_json::from_slice::<Value>(s.borrow().resources[&out].bytes().unwrap()).unwrap()
        });
        assert_eq!(first["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(first["index"], 0);
        assert_eq!(rz_release(out), 1);
        let again = rz_register_font(font, request);
        assert_ne!(again, 0);
        assert_eq!(rz_release(again), 1);
        assert_eq!(STATE.with(|s| s.borrow().font_provider.len()), 1);
        let conflict = options("Changed");
        assert_eq!(rz_register_font(font, conflict), 0);
        assert_eq!(rz_release(conflict), 1);
        let bad = input(b"not a font");
        assert_eq!(rz_register_font(bad, request), 0);
        assert_eq!(rz_release(bad), 1);
        let other = input(include_bytes!(
            "../../pdf/tests/fixtures/synthetic-symbol.ttf"
        ));
        while STATE.with(|s| s.borrow().resources.len()) < 8 {
            input(b"x");
        }
        assert_eq!(rz_register_font(other, request), 0);
        assert_eq!(STATE.with(|s| s.borrow().font_provider.len()), 1);
        assert_eq!(rz_dispose(), 1);
        assert_eq!(rz_retained_bytes(), 0);
        assert_eq!(STATE.with(|s| s.borrow().font_provider.len()), 0);
    }
}

pub(crate) fn requirements(s: &State, document: u32, request: &Value) -> ApiResult<Value> {
    let Some(Resource::Document(d, _)) = s.resources.get(&document) else {
        return Err(ApiError::handle("invalid document handle"));
    };
    let page = request
        .get("page")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("invalid page index")?;
    let opts = request.get("renderOptions");
    if opts.is_some_and(|v| {
        !v.is_object()
            || v.as_object().unwrap().keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "annotations" | "includeWidgets" | "viewZoom" | "viewRotation"
                )
            })
    }) {
        return Err("invalid render options".into());
    }
    let boolean = |name: &str| -> ApiResult<bool> {
        match opts.and_then(|v| v.get(name)) {
            None => Ok(true),
            Some(v) => v.as_bool().ok_or("invalid render option".into()),
        }
    };
    let annotations = boolean("annotations")?;
    let include_widgets = boolean("includeWidgets")?;
    let view_zoom = match opts.and_then(|v| v.get("viewZoom")) {
        None => 1.,
        Some(v) => v
            .as_f64()
            .filter(|n| n.is_finite() && (0.1..=4.).contains(n))
            .ok_or("invalid view zoom")? as f32,
    };
    let view_rotation = match opts.and_then(|v| v.get("viewRotation")) {
        None => 0,
        Some(v) => v
            .as_u64()
            .filter(|n| matches!(n, 0 | 90 | 180 | 270))
            .ok_or("invalid view rotation")? as u16,
    };
    let visibility = if d.optional_content()?.is_some() {
        Some(d.optional_content_visibility(&Default::default())?)
    } else {
        None
    };
    let mut collector = render::font_preflight::FontPreflight::new(
        &s.font_provider,
        s.substitute_font.as_ref(),
        limits(),
    )?;
    let mut p = d.get_page(page)?;
    if annotations {
        let options = pdf::page::annotations::AnnotationRenderOptions {
            include_widgets,
            view_zoom,
            view_rotation,
            ..Default::default()
        };
        let list = if let Some(visibility) = visibility {
            p.display_list_with_annotations_and_optional_content(options, &visibility)?
        } else {
            p.display_list_with_annotations(options)?
        };
        list.replay(&mut collector)?;
    } else {
        let list = if let Some(visibility) = visibility {
            p.display_list_with_optional_content(&visibility)?
        } else {
            p.display_list()?
        };
        list.replay(&mut collector)?;
    }
    let redacted = d.security_report().is_some_and(|r| {
        r.authentication != pdf::security::Authentication::Owner && !r.permissions.copy
    });
    let fonts = collector.finish();
    let complete = fonts
        .iter()
        .all(|f| f.missing.is_empty() && f.missing_unicode == 0 && f.needs_shaping == 0);
    let records:Vec<Value>=fonts.iter().enumerate().map(|(i,f)|{
        let preference=|identity:Option<pdf::font::resolver::FontIdentity>|{
            use pdf::font::resolver::FontProvider;
            let metadata=identity.and_then(|id|(0..s.font_provider.len()).find(|i|s.font_provider.identity(*i)==id).map(|i|s.font_provider.metadata(i)));
            let request=pdf::font::resolver::FontRequest{name:&f.name,character:' ',weight:f.weight,italic:f.italic,serif:f.serif,language:f.language.as_deref()};
            metadata.and_then(|m|pdf::font::resolver::font_preference(m,request).ok()).unwrap_or([2,3,1,1,1000])
        };
        let selected:Vec<Value>=f.selected.iter().map(|(identity,count)|{let rank=preference(*identity);match identity{None=>json!({"legacy":true,"count":count,"languageRank":rank[1],"preference":rank}),Some(id)=>json!({"sha256":id.sha256.iter().map(|b|format!("{b:02x}")).collect::<String>(),"index":id.collection_index,"count":count,"languageRank":rank[1],"preference":rank})}}).collect();
        json!({"fontId":format!("font-{i}"),"name":f.name,"language":f.language,"codepoints":if redacted{vec![]}else{f.codepoints.iter().copied().collect::<Vec<_>>()} ,"weight":f.weight,"italic":f.italic,"serif":f.serif,"missingCount":f.missing.len(),"missingCodepoints":if redacted{vec![]}else{f.missing.iter().copied().collect::<Vec<_>>()},"missingUnicode":f.missing_unicode,"needsShaping":f.needs_shaping,"selected":selected})
    }).collect();
    Ok(
        json!({"stamp":{"document":document,"revision":d.revision().to_string(),"fontEpoch":s.font_epoch.to_string()},"page":page,"renderOptions":{"annotations":annotations,"includeWidgets":include_widgets,"viewZoom":view_zoom,"viewRotation":view_rotation},"codepointsRedacted":redacted,"complete":complete,"fonts":records}),
    )
}

#[cfg(test)]
mod preflight_tests {
    use super::*;
    use pdf::{
        document::resource_manager::ResourceKind,
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_reference::PdfReference, pdf_stream::PdfStream},
        parser::syntax::SyntaxParser,
    };
    fn sample() -> Document {
        let d = Document::new().unwrap();
        d.add_page(0).unwrap();
        let mut font=SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(b"<< /Type /Font /Subtype /TrueType /BaseFont /Missing /FirstChar 32 /LastChar 32 /Widths [1000] /FontDescriptor << /Flags 0 >> >>".to_vec())).unwrap()).get_object().unwrap().into_dict().unwrap();
        let cmap=d.try_insert_object(PdfObject::PdfStream(PdfStream::new(Default::default(),b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <65b0> endbfchar".to_vec()))).unwrap();
        font.try_insert(
            "ToUnicode".into(),
            PdfObject::PdfReference(PdfReference::from_id(cmap)),
        )
        .unwrap();
        let id = d.try_insert_object(PdfObject::PdfDict(font)).unwrap();
        let name = d
            .resources()
            .register_existing(0, ResourceKind::Font, id)
            .unwrap();
        d.replace_page_contents(
            0,
            format!("BT /{name} 12 Tf 20 40 Td <20> Tj ET").into_bytes(),
        )
        .unwrap();
        d
    }
    fn input(bytes: &[u8]) -> u32 {
        STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Input(bytes.to_vec()))
                .unwrap()
        })
    }
    #[test]
    fn preflight_is_readonly_scope_registration_is_atomic_and_no_copy_redacts_unicode() {
        STATE.with(|s| *s.borrow_mut() = Default::default());
        let doc = Rc::new(sample());
        let revision = doc.revision().to_string();
        let document = STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Document(doc.clone(), 0))
                .unwrap()
        });
        let query = json!({"page":0,"renderOptions":{}});
        let before = STATE.with(|s| requirements(&s.borrow(), document, &query).unwrap());
        assert_eq!(before["fonts"][0]["missingCodepoints"], json!([0x65b0]));
        assert_eq!(doc.revision().to_string(), revision);
        let font = input(include_bytes!(
            "../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf"
        ));
        let request=input(&serde_json::to_vec(&json!({"family":"CJK","weight":400,"italic":false,"serif":false,"index":0,"document":document,"revision":revision})).unwrap());
        let reply = rz_register_font(font, request);
        assert_ne!(reply, 0);
        rz_release(reply);
        let ready = STATE.with(|s| requirements(&s.borrow(), document, &query).unwrap());
        assert_eq!(ready["complete"], true);
        assert_eq!(doc.revision().to_string(), revision);
        let scoped=input(&serde_json::to_vec(&json!({"family":"CJK","weight":400,"italic":false,"serif":false,"index":0,"document":document,"revision":revision,"fontEpoch":ready["stamp"]["fontEpoch"]})).unwrap());
        assert_eq!(rz_clear_fonts(), 1);
        assert_eq!(rz_register_font(font, scoped), 0);
        assert_eq!(api::rz_error_code(), 8);
        assert_eq!(STATE.with(|s| s.borrow().font_provider.byte_len()), 0);
        rz_release(scoped);
        doc.rotate_page(0, 90).unwrap();
        assert_eq!(rz_register_font(font, request), 0);
        assert_eq!(api::rz_error_code(), 8);
        rz_release(document);
        assert_eq!(rz_register_font(font, request), 0);
        assert_eq!(api::rz_error_code(), 8);
        rz_dispose();
        let source = sample();
        let mut encryption = pdf::security::EncryptionOptions::new("user", "owner").unwrap();
        encryption.permissions.copy = false;
        let bytes = source
            .save_to_bytes_with_security(
                Default::default(),
                pdf::security::SaveSecurity::Set(&encryption),
            )
            .unwrap();
        let user = Document::from_bytes_with_password(bytes, "user").unwrap();
        let document = STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Document(Rc::new(user), 0))
                .unwrap()
        });
        let report = STATE.with(|s| requirements(&s.borrow(), document, &query).unwrap());
        assert_eq!(report["codepointsRedacted"], true);
        assert_eq!(report["fonts"][0]["missingCodepoints"], json!([]));
        assert_eq!(report["fonts"][0]["codepoints"], json!([]));
        assert_eq!(report["fonts"][0]["missingCount"], 1);
        rz_dispose();
    }
}
