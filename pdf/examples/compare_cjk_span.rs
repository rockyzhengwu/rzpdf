//! Fixed real-font span comparison: INPUT.pdf OUTPUT.pdf
//! [--partial|--same-chain|--mixed-font|--tj|--tj-inside].
//! Input is create_cjk --images. Pages: baseline, original second page,
//! compensated cross-call edit, ordinary cross-call edit.
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream},
    page::{content_builder::ContentBuilder, source_provenance::NestedTextCodePosition},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

fn form(resources: PdfDict, bytes: Vec<u8>) -> PdfResult<PdfObject> {
    let mut dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
        b"<< /Type /XObject /Subtype /Form /BBox [0 0 600 800] >>".as_slice(),
    ))?)
    .get_object()?
    .into_dict()
    .ok_or_else(|| PdfError::ObjectError("Form dictionary absent".into()))?;
    dict.insert("Resources".into(), PdfObject::PdfDict(resources));
    Ok(PdfObject::PdfStream(PdfStream::from_decoded_data(
        dict, bytes,
    )?))
}

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let tj_inside = args.get(2).and_then(|arg| arg.to_str()) == Some("--tj-inside");
    let tj = tj_inside || args.get(2).and_then(|arg| arg.to_str()) == Some("--tj");
    let partial = tj || args.get(2).and_then(|arg| arg.to_str()) == Some("--partial");
    let mixed_font = args.get(2).and_then(|arg| arg.to_str()) == Some("--mixed-font");
    let same_chain = mixed_font || args.get(2).and_then(|arg| arg.to_str()) == Some("--same-chain");
    if args.len() != 2 + usize::from(partial || same_chain) {
        return Err(PdfError::DocumentError(
            "Usage: compare_cjk_span INPUT.pdf OUTPUT.pdf [--partial|--same-chain|--mixed-font|--tj|--tj-inside]"
                .into(),
        ));
    }
    let doc = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    if doc.try_page_count()? != 2 {
        return Err(PdfError::DocumentError(
            "Expected two-page CJK fixture".into(),
        ));
    }
    let page = doc.get_page(0)?;
    let original = page.content_program()?;
    let font = original
        .operations()
        .iter()
        .find(|op| op.operator.name() == "Tf")
        .ok_or_else(|| PdfError::FontError("Fixture font missing".into()))?
        .operator
        .get_as_name(0)?
        .name();
    let sentence = page.encode_text_for_font(font, "你好，世界！Hello PDF")?;
    let end_offset = if partial {
        page.encode_text_for_font(font, "你好，世界！")?.len()
    } else {
        sentence.len()
    };
    let expected = if partial {
        "你好！Hello PDF PDF"
    } else {
        "你好！ PDF"
    };
    let retained = if partial {
        "Hello PDF PDF".chars().count()
    } else {
        3
    };
    let tail = page.encode_text_for_font(font, " PDF")?;
    let mut shows = ContentBuilder::new();
    let tj_array = if tj {
        // Two strings in one TJ event, with both signs of numeric adjustment.
        // Select either the end or an internal code boundary of item zero.
        let prefix = page.encode_text_for_font(
            font,
            if tj_inside {
                "你好，世界！Hello"
            } else {
                "你好，世界！"
            },
        )?;
        let suffix =
            page.encode_text_for_font(font, if tj_inside { " PDF" } else { "Hello PDF" })?;
        let hex =
            |codes: &[u8]| -> String { codes.iter().map(|code| format!("{code:02X}")).collect() };
        // ContentBuilder has no arbitrary multi-item TJ method; preserve the
        // exact fixture syntax instead of simulating this with separate Tj.
        Some(format!(
            "[<{}> -125 <{}> 75] TJ\n",
            hex(&prefix),
            hex(&suffix)
        ))
    } else {
        shows.show_encoded_text(&sentence)?;
        None
    };
    shows.show_encoded_text(&tail)?;
    let mut body = format!("BT /{font} 24 Tf 1 0 0 1 70 650 Tm ").into_bytes();
    if let Some(array) = tj_array {
        body.extend_from_slice(array.as_bytes());
    }
    body.extend_from_slice(shows.as_bytes());
    body.extend_from_slice(b" ET");
    let (leaf_scope, leaf_body) = if mixed_font {
        let latin = doc
            .resources()
            .register_standard_font(0, pdf::document::resource_manager::StandardFont::Helvetica)?;
        // This is intentionally a distinct local font scope and metrics.
        let leaf_scope = doc.get_page(0)?.resource()?;
        (
            leaf_scope,
            format!("BT /{latin} 18 Tf 2 Tc 1 0 0 1 70 650 Tm (Hello PDF) Tj ET").into_bytes(),
        )
    } else {
        (page.resource()?, body.clone())
    };
    let leaf = doc.try_insert_object(form(leaf_scope, leaf_body)?)?;
    let mut xobjects = PdfDict::default();
    xobjects.insert(
        "Inner".into(),
        PdfObject::PdfReference(PdfReference::from_id(leaf)),
    );
    let mut scope = if same_chain {
        page.resource()?
    } else {
        PdfDict::default()
    };
    scope.insert("XObject".into(), PdfObject::PdfDict(xobjects));
    let outer_body = if same_chain {
        let mut bytes = body.clone();
        bytes.extend_from_slice(b" q 1 0 0 1 0 -100 cm /Inner Do Q q 1 0 0 1 0 -200 cm ");
        bytes.extend_from_slice(&body);
        bytes.extend_from_slice(b" Q");
        bytes
    } else {
        b"q /Inner Do Q q 1 0 0 1 0 -100 cm /Inner Do Q".to_vec()
    };
    let outer = doc.try_insert_object(form(scope, outer_body)?)?;
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)?;
    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())?;
    doc.copy_page(0, 2)?;
    doc.copy_page(0, 3)?;
    let baseline = doc.get_page(0)?.content_program()?;
    let before = doc.get_page(0)?.extract_characters_paint_order()?;
    let last = before.len() - retained;
    for (index, preserve) in [(2, true), (3, false)] {
        let root = doc.get_page(index)?.content_program()?;
        let chars = doc.get_page(index)?.extract_characters_paint_order()?;
        let starts: Vec<_> = chars
            .iter()
            .filter(|ch| ch.unicode.as_deref() == Some("你"))
            .collect();
        if starts.len() != if same_chain && !mixed_font { 3 } else { 2 } {
            return Err(PdfError::FontError("Unexpected CJK event count".into()));
        }
        let start = NestedTextCodePosition {
            source: starts[0].nested_source.as_ref().unwrap(),
            array_item: tj.then_some(0),
            offset: 0,
        };
        let end = NestedTextCodePosition {
            source: starts[if same_chain && !mixed_font { 2 } else { 1 }]
                .nested_source
                .as_ref()
                .unwrap(),
            array_item: tj.then_some(0),
            offset: end_offset,
        };
        if same_chain
            && (start.source.invocations != end.source.invocations
                || !chars.iter().any(|ch| {
                    ch.nested_source.as_ref().is_some_and(|source| {
                        source.invocations.len() > start.source.invocations.len()
                    })
                }))
        {
            return Err(PdfError::DocumentError(
                "Expected parent endpoints around child Form".into(),
            ));
        }
        if preserve {
            doc.replace_nested_text_span_preserving_advance(index, &root, start..end, "你好！")?;
        } else {
            doc.replace_nested_text_span(index, &root, start..end, "你好！")?;
        }
        let after = doc.get_page(index)?.extract_characters_paint_order()?;
        if after
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>()
            != expected
        {
            return Err(PdfError::DocumentError("Cross-call text mismatch".into()));
        }
        for (old, new) in before[last..].iter().zip(&after[after.len() - retained..]) {
            let dx = (old.position.x() - new.position.x()).abs();
            let dy = (old.position.y() - new.position.y()).abs();
            if (preserve && (dx > 0.002 || dy > 0.002)) || (!preserve && dx < 10.0) {
                return Err(PdfError::DocumentError(
                    "Cross-call advance comparison failed".into(),
                ));
            }
        }
    }
    if doc.get_page(0)?.content_program()?.source_bytes() != baseline.source_bytes() {
        return Err(PdfError::DocumentError("Shared baseline changed".into()));
    }
    doc.save_to_file(&args[1])?;
    let reopened = PdfDocument::open_with_mode(&args[1], ParseMode::Strict)?;
    for index in 0..4 {
        reopened.get_page(index)?.display_list()?;
    }
    Ok(())
}
