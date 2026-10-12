//! Usage: edit_subset FONT.ttf NAME CAP_HEIGHT STEM_V OUTPUT.pdf [--cjk] [--tj|--events] [--form] [--vertical]
//! Three pages: shared baseline, natural advance, compensated new characters.
use pdf::{budget::OperationBudget, document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions, subset_text_edit::SubsetTextEditOptions}, error::{PdfError, PdfResult}, page::page_builder::PageBuilder, parser::syntax::ParseMode};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 5 || args[5..].iter().any(|flag| flag != "--cjk" && flag != "--tj" && flag != "--events" && flag != "--form" && flag != "--vertical") {
        return Err(PdfError::DocumentError("Usage: edit_subset FONT.ttf NAME CAP_HEIGHT STEM_V OUTPUT.pdf [--cjk] [--tj|--events]".into()));
    }
    let source = std::fs::read(&args[0]).map_err(|error| PdfError::DocumentError(error.to_string()))?;
    let metric = |value: &str| value.parse::<f64>().map_err(|_| PdfError::FontError("Invalid supplied metric".into()));
    let cap = metric(&args[2])?;
    let stem = metric(&args[3])?;
    let embedding = || TrueTypeEmbeddingOptions { postscript_name: args[1].clone(), cap_height: cap, stem_v: stem, serif: false };
    let cjk = args[5..].iter().any(|flag| flag == "--cjk");
    let tj = args[5..].iter().any(|flag| flag == "--tj");
    let events = args[5..].iter().any(|flag| flag == "--events");
    let form = args[5..].iter().any(|flag| flag == "--form");
    let vertical = args[5..].iter().any(|flag| flag == "--vertical");
    if form && !events { return Err(PdfError::DocumentError("--form currently requires --events".into())); }
    if tj && events { return Err(PdfError::DocumentError("Choose --tj or --events, not both".into())); }
    let (initial, replacement, range, edited) = if cjk { ("你你你", "好", 2..6, "你好") } else { ("A A A", "éÅ", 4..10, "A éÅ") };
    let doc = PdfDocument::new()?;
    for page in 0..3 { doc.add_page_with_builder(page, PageBuilder::new().size(595.0, 842.0))?; }
    let pool = OperationBudget::new(128 * 1024 * 1024);
    let original = if vertical {
        doc.resources().embed_truetype_vertical_subset_font(0, &source, initial, embedding(), *b"EDITAA", true, &pool)?
    } else {
        doc.resources().embed_truetype_subset_font(0, &source, initial, embedding(), *b"EDITAA", true, &pool)?
    };
    let mut shared_form = None;
    for page in 0..3 {
        let name = if page == 0 { original.font.resource_name.clone() } else { original.font.register_on_page(&doc, page)? };
        let codes = original.font.encoding().encode_text(initial)?;
        let mut content = pdf::page::content_builder::ContentBuilder::new();
        content.begin_text().try_set_font(&name, 24.0)?;
        content.move_text(48.0, 760.0);
        if events {
            let split = if cjk { [2, 4] } else { [6, 8] };
            let hex = |bytes: &[u8]| bytes.iter().map(|byte| format!("{byte:02X}")).collect::<String>();
            content.show_encoded_text(&codes[..split[0]])?;
            content.raw_operator(&format!("[<{}> -25 % before\n <{}> +15 % after\n] TJ", hex(&codes[split[0]..split[1]]), hex(&codes[split[1]..])));
        } else if tj {
            let split = if cjk { [2, 4] } else { [6, 8] };
            let hex = |bytes: &[u8]| bytes.iter().map(|byte| format!("{byte:02X}")).collect::<String>();
            content.raw_operator(&format!("[<{}> -25 % before\n <{}> +15 % after\n <{}>] TJ", hex(&codes[..split[0]]), hex(&codes[split[0]..split[1]]), hex(&codes[split[1]..])));
        } else {
            content.show_encoded_text(&codes)?;
        }
        content.show_encoded_text(&codes)?;
        content.end_text();
        if form {
            use pdf::{io::stream_reader::StreamReader, objects::{PdfObject, pdf_stream::PdfStream}, parser::syntax::SyntaxParser, document::resource_manager::ResourceKind};
            let handle = if let Some(handle) = shared_form { handle } else {
                let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                    b"<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] >>".as_slice()
                ))?).get_object()?.into_dict().ok_or_else(|| PdfError::ObjectError("Form dictionary absent".into()))?;
                dictionary.insert("Resources".into(), PdfObject::PdfDict(doc.get_page(0)?.resource()?));
                let handle = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary, content.build())?))?;
                shared_form = Some(handle);
                handle
            };
            let name = doc.resources().register_existing(page, ResourceKind::Form, handle)?;
            doc.replace_page_contents(page, format!("/{name} Do").into_bytes())?;
        } else {
            doc.replace_page_content_builder(page, &content)?;
        }
    }
    for (page, preserve, tag) in [(1, false, *b"EDITAB"), (2, true, *b"EDITAC")] {
        let snapshot = doc.get_page(page)?.content_program()?;
        let nested = if form {
            doc.get_page(page)?.extract_characters_paint_order_report()?.characters.first()
                .and_then(|character| character.nested_source.clone())
        } else { None };
        let leaf = nested.as_ref().map(|source| doc.get_page(page)?.nested_content_program(&snapshot, source)).transpose()?;
        let program = leaf.as_ref().unwrap_or(&snapshot);
        let operation = program.operations().iter().position(|op| op.operator.name() == if tj { "TJ" } else { "Tj" }).ok_or_else(|| PdfError::ObjectError("Missing fixture text".into()))?;
        let options = SubsetTextEditOptions { source: &source, embedding: embedding(), subset_tag: tag, omit_layout_and_auxiliary: true, preserve_advance: preserve };
        if events {
            use pdf::page::content_program::TextCodePosition;
            let end = program.operations().iter().position(|op| op.operator.name() == "TJ").ok_or_else(|| PdfError::ObjectError("Missing fixture tail event".into()))?;
            let selected = TextCodePosition { operation, array_item: None, offset: range.start }..TextCodePosition { operation: end, array_item: Some(2), offset: 2 };
            if form {
                doc.replace_nested_text_event_range_with_subset_and_budget(page, &snapshot, nested.as_ref().ok_or_else(|| PdfError::ObjectError("Missing Form provenance".into()))?, selected, replacement, options, &pool)?;
            } else {
                doc.replace_page_text_event_range_with_subset_and_budget(page, &snapshot, selected, replacement, options, &pool)?;
            }
        } else if tj {
            doc.replace_page_text_array_range_with_subset_and_budget(page, &snapshot, operation, (0, range.start)..(4, 2), replacement, options, &pool)?;
        } else {
            doc.replace_page_text_string_with_subset_and_budget(page, &snapshot, operation, range.clone(), replacement, options, &pool)?;
        }
        if tj || events {
            let current_root = doc.get_page(page)?.content_program()?;
            let current = if form {
                let report = doc.get_page(page)?.extract_characters_paint_order_report()?;
                doc.get_page(page)?.nested_content_program(&current_root, report.characters[0].nested_source.as_ref().ok_or_else(|| PdfError::ObjectError("Missing edited Form source".into()))?)?
            } else { current_root };
            for token in [b"-25 % before".as_slice(), b"+15 % after".as_slice()] {
                if !current.source_bytes().windows(token.len()).any(|bytes| bytes == token) {
                    return Err(PdfError::ObjectError("TJ numeric/comment bytes changed".into()));
                }
            }
        }
    }
    doc.save_to_file(&args[4])?;
    let reopened = PdfDocument::open_with_mode(&args[4], ParseMode::Strict)?;
    for (page, expected) in [(0, format!("{initial}{initial}")), (1, format!("{edited}{initial}")), (2, format!("{edited}{initial}"))] {
        let chars = reopened.get_page(page)?.extract_characters_paint_order()?;
        if chars.iter().filter_map(|c| c.unicode.as_deref()).collect::<String>() != expected {
            return Err(PdfError::ObjectError("Edited fixture extraction mismatch".into()));
        }
    }
    eprintln!("Tracked operation peak: {} bytes", pool.peak_bytes());
    Ok(())
}
