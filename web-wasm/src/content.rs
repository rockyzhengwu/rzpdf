//! Source-authenticated content edits; no serialized native addresses are trusted.
use super::*;
use pdf::{
    document::{identity::ObjectHandle, page_transfer::PageTransferOptions},
    error::{PdfError, PdfResult},
    geom::matrix::Matrix,
    objects::PdfObject,
    page::{
        page_object::PageObject,
        source_provenance::{NestedDrawSource, NestedTextCodePosition},
        text_extraction::ExtractedCharacter,
    },
};
use serde_json::{Value, json};
fn invalid() -> ApiError {
    ApiError::new(9, "invalid content edit request")
}
fn uint(v: &Value) -> ApiResult<u32> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(invalid)
}
fn text(v: &Value) -> ApiResult<String> {
    v.as_str()
        .filter(|s| s.len() <= 16 * 1024)
        .map(str::to_owned)
        .ok_or_else(invalid)
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
fn matrix(v: &Value) -> ApiResult<[f32; 6]> {
    let values = v.as_array().filter(|a| a.len() == 6).ok_or_else(invalid)?;
    let mut out = [0.; 6];
    for (i, n) in values.iter().enumerate() {
        out[i] = n
            .as_f64()
            .map(|n| n as f32)
            .filter(|n| n.is_finite())
            .ok_or_else(invalid)?;
    }
    Ok(out)
}
fn chars(d: &Document, page: u32, enforce_copy: bool) -> PdfResult<Vec<ExtractedCharacter>> {
    if enforce_copy
        && d.security_report().is_some_and(|r| {
            r.authentication == pdf::security::Authentication::User && !r.permissions.copy
        })
    {
        return Err(PdfError::PermissionDenied("content extraction".into()));
    }
    let mut p = d.get_page(page)?;
    Ok(if d.optional_content()?.is_some() {
        p.extract_characters_with_optional_content_report(
            &d.optional_content_visibility(&Default::default())?,
        )?
    } else {
        p.extract_characters_paint_order_report()?
    }
    .characters)
}
fn images(d: &Document, page: u32) -> PdfResult<Vec<pdf::page::image_object::ImageObject>> {
    let mut p = d.get_page(page)?;
    let list = if d.optional_content()?.is_some() {
        p.display_list_with_optional_content(&d.optional_content_visibility(&Default::default())?)?
    } else {
        p.display_list()?
    };
    Ok(list
        .into_objects()
        .into_iter()
        .filter_map(|o| {
            if let PageObject::Image(i) = o {
                Some(i)
            } else {
                None
            }
        })
        .collect())
}
fn source(c: &ExtractedCharacter) -> PdfResult<NestedDrawSource> {
    c.nested_source
        .clone()
        .or_else(|| c.source.as_ref().map(NestedDrawSource::from_page_text))
        .ok_or_else(|| PdfError::UnsupportedFeature("character has no editable source".into()))
}
fn matrix_json(m: &Matrix) -> [f32; 6] {
    [m.a(), m.b(), m.c(), m.d(), m.e(), m.f()]
}
pub(crate) fn query(d: &Document, handle: u32, page: u32) -> ApiResult<Value> {
    let cs = chars(d, page, true)?;
    let mut bytes = 0usize;
    let mut characters = Vec::new();
    for (index, c) in cs.iter().enumerate() {
        bytes = bytes.saturating_add(
            512 + c.unicode.as_ref().map_or(0, String::len)
                + c.font_name.as_ref().map_or(0, String::len),
        );
        if bytes > 8 * 1024 * 1024 {
            return Err(ApiError::resource("editable content metadata budget"));
        }
        characters.push(json!({"index":index,"unicode":c.unicode,"objectIndex":c.object_index,"position":[c.position.x(),c.position.y()],"advance":c.advance,"fontName":c.font_name,"emToDisplay":c.em_to_display_matrix().ok().as_ref().map(matrix_json),"editable":(c.source.is_some()||c.nested_source.is_some())&&c.decoded_code_range.is_some(),"nested":c.nested_source.is_some()}));
    }
    let mut records = Vec::new();
    for (index, i) in images(d, page)?.iter().enumerate() {
        bytes = bytes.saturating_add(512);
        if bytes > 8 * 1024 * 1024 {
            return Err(ApiError::resource("editable content metadata budget"));
        }
        records.push(json!({"index":index,"matrix":matrix_json(&i.matrix),"editable":i.source().is_some()||i.nested_source().is_some(),"nested":i.nested_source().is_some()}));
    }
    Ok(
        json!({"stamp":{"document":handle,"revision":d.revision().to_string()},"page":page,"characters":characters,"images":records,"coordinateSpace":"display-points-top-left"}),
    )
}
pub(crate) enum Command {
    Text {
        page: u32,
        start: usize,
        end: usize,
        text: String,
        preserve: bool,
        span: bool,
        font: Option<FontInput>,
    },
    Image {
        page: u32,
        index: usize,
        edit: ImageEdit,
    },
    Rotate(u32, i32),
    Copy(u32, u32, String),
    Delete(u32),
    Reorder(Vec<u32>),
}
pub(crate) struct FontInput {
    embedding: pdf::document::embedded_font::TrueTypeEmbeddingOptions,
    omit: bool,
}
fn parse_font(value: &Value) -> ApiResult<FontInput> {
    keys(
        value,
        &[
            "postscriptName",
            "capHeight",
            "stemV",
            "serif",
            "omitLayoutAndAuxiliary",
        ],
    )?;
    let numeric = |key: &str| {
        value[key]
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(invalid)
    };
    Ok(FontInput {
        embedding: pdf::document::embedded_font::TrueTypeEmbeddingOptions {
            postscript_name: text(&value["postscriptName"])?,
            cap_height: numeric("capHeight")?,
            stem_v: numeric("stemV")?,
            serif: value["serif"].as_bool().ok_or_else(invalid)?,
        },
        omit: value
            .get("omitLayoutAndAuxiliary")
            .map(|v| v.as_bool().ok_or_else(invalid))
            .transpose()?
            .unwrap_or(false),
    })
}
pub(crate) enum ImageEdit {
    Remove,
    Transform([f32; 6]),
    Replace(String),
}
pub(crate) fn prepare(r: &Value) -> ApiResult<Command> {
    keys(
        r,
        &[
            "operation",
            "stamp",
            "page",
            "start",
            "end",
            "text",
            "preserveAdvance",
            "font",
            "index",
            "matrix",
            "format",
            "degrees",
            "insertion",
            "namePrefix",
            "order",
        ],
    )?;
    Ok(match r["operation"].as_str().ok_or_else(invalid)? {
        "replaceText" | "replaceTextSpan" => Command::Text {
            page: uint(&r["page"])?,
            start: uint(&r["start"])? as usize,
            end: uint(&r["end"])? as usize,
            text: text(&r["text"])?,
            span: r["operation"] == "replaceTextSpan",
            font: r.get("font").map(parse_font).transpose()?,
            preserve: r
                .get("preserveAdvance")
                .map(|v| v.as_bool().ok_or_else(invalid))
                .transpose()?
                .unwrap_or(true),
        },
        "removeImage" | "transformImage" | "replaceImage" => Command::Image {
            page: uint(&r["page"])?,
            index: uint(&r["index"])? as usize,
            edit: match r["operation"].as_str().unwrap() {
                "removeImage" => ImageEdit::Remove,
                "transformImage" => ImageEdit::Transform(matrix(&r["matrix"])?),
                _ => {
                    let format = text(&r["format"])?;
                    if !["png", "jpeg"].contains(&format.as_str()) {
                        return Err(invalid());
                    }
                    ImageEdit::Replace(format)
                }
            },
        },
        "rotatePage" => Command::Rotate(
            uint(&r["page"])?,
            r["degrees"]
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .filter(|n| n % 90 == 0)
                .ok_or_else(invalid)?,
        ),
        "copyPage" => Command::Copy(
            uint(&r["page"])?,
            uint(&r["insertion"])?,
            r.get("namePrefix")
                .map(text)
                .transpose()?
                .unwrap_or_default(),
        ),
        "deletePage" => Command::Delete(uint(&r["page"])?),
        "reorderPages" => Command::Reorder(
            r["order"]
                .as_array()
                .filter(|a| a.len() <= 10000)
                .ok_or_else(invalid)?
                .iter()
                .map(uint)
                .collect::<ApiResult<_>>()?,
        ),
        _ => return Err(invalid()),
    })
}
impl Command {
    pub(crate) fn asset_limit(&self) -> Option<usize> {
        match self {
            Self::Text { font: Some(_), .. } => Some(FONT_LIMIT),
            Self::Image {
                edit: ImageEdit::Replace(_),
                ..
            } => Some(INPUT_LIMIT),
            _ => None,
        }
    }
}
fn resolved(d: &Document, mut value: PdfObject) -> PdfResult<PdfObject> {
    for _ in 0..64 {
        if let Some(r) = value.as_reference() {
            value = d.resolve_object(r.id())?;
        } else {
            return Ok(value);
        }
    }
    Err(PdfError::ResourceLimit(
        "image resource reference depth".into(),
    ))
}
fn replacement(d: &Document, page: u32, format: &str, bytes: &[u8]) -> PdfResult<ObjectHandle> {
    let name = if format == "png" {
        d.resources().embed_png_image(page, bytes.to_vec())?
    } else {
        d.resources().embed_jpeg_image(page, bytes.to_vec())?
    };
    let resources = d.get_page(page)?.resource()?;
    let category = resolved(
        d,
        resources
            .get("XObject")
            .ok_or_else(|| PdfError::PageResourceError("image category absent".into()))?
            .clone(),
    )?;
    let id = category
        .as_dict()
        .and_then(|dict| dict.get(&name))
        .and_then(|v| v.as_reference())
        .ok_or_else(|| PdfError::PageResourceError("new image resource absent".into()))?
        .id();
    d.object_handle(id)
}
pub(crate) fn apply(
    d: &Document,
    command: Command,
    asset: Option<&[u8]>,
    budget: &pdf::budget::OperationBudget,
) -> PdfResult<Option<pdf::objects::object_id::ObjectId>> {
    match command {
        Command::Text {
            page,
            start,
            end,
            text,
            preserve,
            span,
            font,
        } => {
            let cs = chars(d, page, false)?;
            if start >= end || end > cs.len() {
                return Err(PdfError::ObjectError("text selection out of bounds".into()));
            }
            let first = &cs[start];
            let last = &cs[end - 1];
            if !span
                && cs[start..end]
                    .iter()
                    .any(|c| c.object_index != first.object_index)
            {
                return Err(PdfError::UnsupportedFeature(
                    "select one text draw event; cross-event edits require an explicit span".into(),
                ));
            }
            let a = source(first)?;
            let b = source(last)?;
            if !span && a != b {
                return Err(PdfError::UnsupportedFeature(
                    "text selection crosses source invocations".into(),
                ));
            }
            let begin = NestedTextCodePosition {
                source: &a,
                array_item: first.array_item_index,
                offset: first
                    .decoded_code_range
                    .as_ref()
                    .ok_or_else(|| {
                        PdfError::UnsupportedFeature("text code boundary absent".into())
                    })?
                    .start,
            };
            let finish = NestedTextCodePosition {
                source: &b,
                array_item: last.array_item_index,
                offset: last
                    .decoded_code_range
                    .as_ref()
                    .ok_or_else(|| {
                        PdfError::UnsupportedFeature("text code boundary absent".into())
                    })?
                    .end,
            };
            if span && d.optional_content()?.is_some() {
                let all = d.get_page(page)?.extract_characters_paint_order()?;
                let same = |x: &ExtractedCharacter, y: &ExtractedCharacter| {
                    x.source == y.source
                        && x.nested_source == y.nested_source
                        && x.array_item_index == y.array_item_index
                        && x.decoded_code_range == y.decoded_code_range
                };
                let lo = all.iter().position(|c| same(c, first)).ok_or_else(|| {
                    PdfError::UnsupportedFeature("span start is not in original source".into())
                })?;
                let hi = all.iter().rposition(|c| same(c, last)).ok_or_else(|| {
                    PdfError::UnsupportedFeature("span end is not in original source".into())
                })?;
                if lo > hi
                    || hi - lo + 1 != end - start
                    || !all[lo..=hi]
                        .iter()
                        .zip(&cs[start..end])
                        .all(|(x, y)| same(x, y))
                {
                    return Err(PdfError::UnsupportedFeature(
                        "span crosses hidden optional-content text".into(),
                    ));
                }
            }
            let root = d.get_page(page)?.content_program()?;
            if let Some(font) = font {
                let tag = d.resources().next_subset_tag_with_budget(budget)?;
                let options = pdf::document::subset_text_edit::SubsetTextEditOptions {
                    source: asset.ok_or_else(|| {
                        PdfError::FontError("full TrueType source required".into())
                    })?,
                    embedding: font.embedding,
                    subset_tag: tag,
                    omit_layout_and_auxiliary: font.omit,
                    preserve_advance: preserve,
                };
                d.replace_nested_text_span_with_subset_and_budget(
                    page,
                    &root,
                    begin..finish,
                    &text,
                    options,
                    budget,
                )?;
            } else if preserve {
                d.replace_nested_text_span_preserving_advance(page, &root, begin..finish, &text)?;
            } else {
                d.replace_nested_text_span(page, &root, begin..finish, &text)?;
            }
        }
        Command::Image { page, index, edit } => {
            let selected = images(d, page)?
                .get(index)
                .cloned()
                .ok_or_else(|| PdfError::ObjectError("image selection out of bounds".into()))?;
            // Register first, then obtain a fresh root snapshot: resource creation advances revision.
            let replacement = if let ImageEdit::Replace(ref format) = edit {
                Some(replacement(
                    d,
                    page,
                    format,
                    asset.ok_or_else(|| {
                        PdfError::ObjectError("replacement image bytes absent".into())
                    })?,
                )?)
            } else {
                None
            };
            let root = d.get_page(page)?.content_program()?;
            if let Some(source) = selected.nested_source() {
                match edit {
                    ImageEdit::Remove => d.remove_nested_image_draw(page, &root, source)?,
                    ImageEdit::Replace(_) => d.replace_nested_image_draw(
                        page,
                        &root,
                        source,
                        replacement.as_ref().unwrap(),
                    )?,
                    ImageEdit::Transform(values) => {
                        let m = &selected.matrix;
                        let current = Matrix::new(1., 0., 0., -1., 0., 1.).checked_mul(m)?;
                        let display = Matrix::new(
                            values[0], values[1], values[2], values[3], values[4], values[5],
                        );
                        let local = current
                            .checked_mul(&display)?
                            .checked_mul(&current.checked_inverse()?)?;
                        d.transform_nested_image_draw(page, &root, source, matrix_json(&local))?;
                    }
                }
            } else {
                let operation = selected
                    .source()
                    .ok_or_else(|| {
                        PdfError::UnsupportedFeature("image has no editable source".into())
                    })?
                    .operation_index;
                match edit {
                    ImageEdit::Remove => d.remove_page_image_draw(page, &root, operation)?,
                    ImageEdit::Transform(matrix) => {
                        d.transform_page_image_draw_display(page, &root, operation, matrix)?
                    }
                    ImageEdit::Replace(_) => d.replace_page_image_draw(
                        page,
                        &root,
                        operation,
                        replacement.as_ref().unwrap(),
                    )?,
                }
            }
        }
        Command::Rotate(page, degrees) => d.rotate_page(page, degrees)?,
        Command::Copy(page, insertion, prefix) => {
            return Ok(Some(d.copy_page_with_options(
                page,
                insertion,
                &PageTransferOptions {
                    name_prefix: prefix,
                    ..Default::default()
                },
            )?));
        }
        Command::Delete(page) => {
            if d.try_page_count()? <= 1 {
                return Err(PdfError::UnsupportedFeature(
                    "Web cannot remove the last page".into(),
                ));
            }
            d.delete_page(page)?;
        }
        Command::Reorder(order) => d.reorder_pages(&order)?,
    }
    Ok(None)
}
