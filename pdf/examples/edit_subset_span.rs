//! Fixed real-font cross-invocation fixture: baseline, natural, compensated.
//! Usage: edit_subset_span FONT.ttf NAME CAP_HEIGHT STEM_V OUTPUT.pdf [--cjk] [--nested] [--vertical] [--mixed-tail | --cid-tail=FONT.ttf [--tail-vertical]]
use pdf::{
    budget::OperationBudget,
    document::{
        PdfDocument, embedded_font::TrueTypeEmbeddingOptions, resource_manager::ResourceKind,
        subset_text_edit::SubsetTextEditOptions,
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, source_provenance::NestedTextCodePosition},
    parser::syntax::{ParseMode, SyntaxParser},
};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 5
        || args.len() > 10
        || args[5..].iter().any(|flag| {
            flag != "--cjk"
                && flag != "--nested"
                && flag != "--vertical"
                && flag != "--tail-vertical"
                && flag != "--mixed-tail"
                && !flag.starts_with("--cid-tail=")
        })
        || args[5..]
            .iter()
            .filter(|flag| *flag == "--tail-vertical")
            .count()
            > 1
        || args[5..]
            .iter()
            .filter(|flag| *flag == "--vertical")
            .count()
            > 1
        || args[5..].iter().filter(|flag| *flag == "--cjk").count() > 1
        || args[5..].iter().filter(|flag| *flag == "--nested").count() > 1
        || args[5..]
            .iter()
            .filter(|flag| *flag == "--mixed-tail" || flag.starts_with("--cid-tail="))
            .count()
            > 1
    {
        return Err(PdfError::DocumentError(
            "Usage: edit_subset_span FONT.ttf NAME CAP_HEIGHT STEM_V OUTPUT.pdf [--cjk] [--nested] [--vertical] [--mixed-tail | --cid-tail=FONT.ttf [--tail-vertical]]".into(),
        ));
    }
    let cjk = args[5..].iter().any(|flag| flag == "--cjk");
    let nested = args[5..].iter().any(|flag| flag == "--nested");
    let vertical = args[5..].iter().any(|flag| flag == "--vertical");
    // Fixed pair: Arial primary uses NotoSC tail; NotoSC primary uses Arial.
    let cid_tail = args[5..]
        .iter()
        .find_map(|flag| flag.strip_prefix("--cid-tail="));
    let tail_vertical = args[5..].iter().any(|flag| flag == "--tail-vertical");
    if tail_vertical && cid_tail.is_none() {
        return Err(PdfError::DocumentError(
            "--tail-vertical requires --cid-tail=FONT.ttf".into(),
        ));
    }
    let mixed = cid_tail.is_some() || args[5..].iter().any(|flag| flag == "--mixed-tail");
    let source =
        std::fs::read(&args[0]).map_err(|error| PdfError::DocumentError(error.to_string()))?;
    let metric = |value: &str| {
        value
            .parse::<f64>()
            .map_err(|_| PdfError::FontError("Invalid supplied metric".into()))
    };
    let cap_height = metric(&args[2])?;
    let stem_v = metric(&args[3])?;
    let embedding = || TrueTypeEmbeddingOptions {
        postscript_name: args[1].clone(),
        cap_height,
        stem_v,
        serif: false,
    };
    let (initial, replacement, edited) = if cjk {
        ("你你你", "好", "你好你你你你你")
    } else {
        ("AAA", "éÅ", "AéÅAAAAA")
    };
    let doc = PdfDocument::new()?;
    for page in 0..3 {
        doc.add_page_with_builder(page, PageBuilder::new().size(595.0, 842.0))?;
    }
    let pool = OperationBudget::new(128 * 1024 * 1024);
    let resources = doc.resources();
    let register = if vertical {
        pdf::document::resource_manager::ResourceManager::embed_truetype_vertical_subset_font
    } else {
        pdf::document::resource_manager::ResourceManager::embed_truetype_subset_font
    };
    let font = register(
        &resources,
        0,
        &source,
        initial,
        embedding(),
        *b"EDITAA",
        true,
        &pool,
    )?;
    let codes = font.font.encoding().encode_text(initial)?;
    let hex = codes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        b"<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] >>".as_slice(),
    ))?)
    .get_object()?
    .into_dict()
    .ok_or_else(|| PdfError::ObjectError("Form dictionary absent".into()))?;
    dictionary.insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(0)?.resource()?),
    );
    let original = PdfObject::PdfStream(PdfStream::from_decoded_data(
        dictionary,
        format!(
            "BT /{} 24 Tf 1 Tc 48 760 Td <{hex}> Tj ET",
            font.font.resource_name
        )
        .into_bytes(),
    )?);
    let handle = doc.try_insert_object(original.clone())?;
    let parent = if nested {
        let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
            b"<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] >>".as_slice(),
        ))?)
        .get_object()?
        .into_dict()
        .ok_or_else(|| PdfError::ObjectError("Parent Form dictionary absent".into()))?;
        dictionary.insert(
            "Resources".into(),
            PdfObject::PdfDict(PdfDict::new(std::collections::HashMap::from([(
                "XObject".into(),
                PdfObject::PdfDict(PdfDict::new(std::collections::HashMap::from([(
                    "Leaf".into(),
                    PdfObject::PdfReference(PdfReference::from_id(handle)),
                )]))),
            )]))),
        );
        let original_parent = PdfObject::PdfStream(PdfStream::from_decoded_data(
            dictionary,
            b"q /Leaf Do Q".to_vec(),
        )?);
        Some((
            doc.try_insert_object(original_parent.clone())?,
            original_parent,
        ))
    } else {
        None
    };
    let invocation = parent.as_ref().map_or(handle, |(id, _)| *id);
    // The tail uses its own encoding and different actual advances. This must
    // exercise the tail font's deletion plan, not the first font's metrics.
    let mixed_tail = if mixed {
        let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
            b"<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] /Resources << /Font << /Tail << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >> >>".as_slice(),
        ))?).get_object()?.into_dict()
            .ok_or_else(|| PdfError::ObjectError("Tail Form dictionary absent".into()))?;
        let tail_content = if let Some(path) = cid_tail {
            let tail_source =
                std::fs::read(path).map_err(|error| PdfError::DocumentError(error.to_string()))?;
            let tail_resources = doc.resources();
            let register_tail = if tail_vertical {
                pdf::document::resource_manager::ResourceManager::embed_truetype_vertical_subset_font
            } else {
                pdf::document::resource_manager::ResourceManager::embed_truetype_subset_font
            };
            let tail_font = register_tail(
                &tail_resources,
                0,
                &tail_source,
                "BBB",
                TrueTypeEmbeddingOptions {
                    postscript_name: if cjk { "ArialMT" } else { "NotoSansSC-Thin" }.into(),
                    cap_height: if cjk { 716.0 } else { 733.0 },
                    stem_v: 80.0,
                    serif: false,
                },
                *b"TAILAA",
                true,
                &pool,
            )?;
            let tail_codes = tail_font.font.encoding().encode_text("BBB")?;
            let tail_hex = tail_codes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>();
            dictionary.insert(
                "Resources".into(),
                PdfObject::PdfDict(doc.get_page(0)?.resource()?),
            );
            format!(
                "BT /{} 18 Tf 2 Tc 48 760 Td <{tail_hex}> Tj ET",
                tail_font.font.resource_name
            )
            .into_bytes()
        } else {
            b"BT /Tail 18 Tf 2 Tc 48 760 Td (BBB) Tj ET".to_vec()
        };
        let original_tail =
            PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary, tail_content)?);
        let tail = doc.try_insert_object(original_tail.clone())?;
        let tail_parent = if nested {
            let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] >>".as_slice(),
            ))?)
            .get_object()?
            .into_dict()
            .ok_or_else(|| PdfError::ObjectError("Tail parent dictionary absent".into()))?;
            dictionary.insert(
                "Resources".into(),
                PdfObject::PdfDict(PdfDict::new(std::collections::HashMap::from([(
                    "XObject".into(),
                    PdfObject::PdfDict(PdfDict::new(std::collections::HashMap::from([(
                        "Leaf".into(),
                        PdfObject::PdfReference(PdfReference::from_id(tail)),
                    )]))),
                )]))),
            );
            let original_parent = PdfObject::PdfStream(PdfStream::from_decoded_data(
                dictionary,
                b"q /Leaf Do Q".to_vec(),
            )?);
            Some((
                doc.try_insert_object(original_parent.clone())?,
                original_parent,
            ))
        } else {
            None
        };
        let tail_invocation = tail_parent.as_ref().map_or(tail, |(id, _)| *id);
        Some((tail, original_tail, tail_invocation, tail_parent))
    } else {
        None
    };
    for page in 0..3 {
        let name = doc
            .resources()
            .register_existing(page, ResourceKind::Form, invocation)?;
        let tail_name = if let Some((_, _, id, _)) = &mixed_tail {
            doc.resources()
                .register_existing(page, ResourceKind::Form, *id)?
        } else {
            name.clone()
        };
        let (second, third) = if vertical || tail_vertical {
            ("80 0", "160 0")
        } else {
            ("0 -80", "0 -160")
        };
        doc.replace_page_contents(page, format!(
            "q /{name} Do Q q 1 0 0 1 {second} cm /{tail_name} Do Q q 1 0 0 1 {third} cm /{name} Do Q"
        ).into_bytes())?;
    }
    for (page, preserve, tag) in [(1, false, *b"EDITAB"), (2, true, *b"EDITAC")] {
        let root = doc.get_page(page)?.content_program()?;
        let report = doc
            .get_page(page)?
            .extract_characters_paint_order_report()?;
        let position = |index: usize| -> PdfResult<NestedTextCodePosition<'_>> {
            Ok(NestedTextCodePosition {
                source: report.characters[index]
                    .nested_source
                    .as_ref()
                    .ok_or_else(|| PdfError::ObjectError("Missing Form provenance".into()))?,
                array_item: None,
                offset: if mixed && cid_tail.is_none() && index == 3 {
                    1
                } else {
                    2
                },
            })
        };
        let expected_depth = if nested { 2 } else { 1 };
        if report.characters.iter().any(|character| {
            character
                .nested_source
                .as_ref()
                .is_none_or(|source| source.invocations.len() != expected_depth)
        }) {
            return Err(PdfError::ObjectError(
                "Unexpected fixture invocation depth".into(),
            ));
        }
        doc.replace_nested_text_span_with_subset_and_budget(
            page,
            &root,
            position(0)?..position(3)?,
            replacement,
            SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: tag,
                omit_layout_and_auxiliary: true,
                preserve_advance: preserve,
            },
            &pool,
        )?;
    }
    if doc.resolve_object(handle)? != original {
        return Err(PdfError::ObjectError("Original shared Form changed".into()));
    }
    if let Some((id, original_tail, _, tail_parent)) = mixed_tail {
        if doc.resolve_object(id)? != original_tail {
            return Err(PdfError::ObjectError(
                "Original mixed tail Form changed".into(),
            ));
        }
        if let Some((parent_id, original_parent)) = tail_parent
            && doc.resolve_object(parent_id)? != original_parent
        {
            return Err(PdfError::ObjectError(
                "Original mixed tail parent Form changed".into(),
            ));
        }
    }
    if let Some((id, original_parent)) = parent
        && doc.resolve_object(id)? != original_parent
    {
        return Err(PdfError::ObjectError(
            "Original shared parent Form changed".into(),
        ));
    }
    doc.save_to_file(&args[4])?;
    let reopened = PdfDocument::open_with_mode(&args[4], ParseMode::Strict)?;
    let baseline = if mixed {
        format!("{initial}BBB{initial}")
    } else {
        initial.repeat(3)
    };
    let edited = if mixed {
        format!("{}{replacement}BB{initial}", if cjk { "你" } else { "A" })
    } else {
        edited.into()
    };
    for (page, expected) in [(0, baseline), (1, edited.clone()), (2, edited)] {
        let chars = reopened.get_page(page)?.extract_characters_paint_order()?;
        if chars
            .iter()
            .filter_map(|character| character.unicode.as_deref())
            .collect::<String>()
            != expected
        {
            return Err(PdfError::ObjectError(
                "Cross-call fixture text mismatch".into(),
            ));
        }
    }
    eprintln!("Tracked operation peak: {} bytes", pool.peak_bytes());
    Ok(())
}
