//! Usage: compare_cjk_advance INPUT.pdf OUTPUT.pdf
//! Fixed create_cjk --images fixture. Output: baseline, second source page,
//! compensated shorter text, uncompensated shorter text. The trailing PDF event
//! must keep its position only on the compensated page.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    page::content_builder::ContentBuilder,
    parser::syntax::ParseMode,
};
use std::io::{Read, Seek};

fn following_positions<R: Read + Seek>(
    doc: &PdfDocument<R>,
    index: u32,
) -> PdfResult<Vec<(f32, f32)>> {
    let characters = doc.get_page(index)?.extract_characters_paint_order()?;
    let start = characters
        .len()
        .checked_sub(3)
        .ok_or_else(|| PdfError::FontError("Trailing PDF missing".into()))?;
    let tail = characters
        .get(start..)
        .ok_or_else(|| PdfError::FontError("Trailing PDF missing".into()))?;
    if tail
        .iter()
        .filter_map(|ch| ch.unicode.as_deref())
        .collect::<String>()
        != "PDF"
    {
        return Err(PdfError::FontError("Trailing PDF mismatch".into()));
    }
    Ok(tail
        .iter()
        .map(|ch| (ch.position.x(), ch.position.y()))
        .collect())
}

fn check<R: Read + Seek>(doc: &PdfDocument<R>) -> PdfResult<()> {
    if doc.try_page_count()? != 4 {
        return Err(PdfError::DocumentError(
            "Expected four comparison pages".into(),
        ));
    }
    let baseline = following_positions(doc, 0)?;
    let adjusted = following_positions(doc, 2)?;
    let unadjusted = following_positions(doc, 3)?;
    for ((expected, actual), control) in baseline.iter().zip(&adjusted).zip(&unadjusted) {
        if (expected.0 - actual.0).abs() > 0.002
            || (expected.1 - actual.1).abs() > 0.002
            || (expected.0 - control.0).abs() < 10.0
        {
            return Err(PdfError::DocumentError("Advance comparison failed".into()));
        }
    }
    for index in 0..4 {
        doc.get_page(index)?.display_list()?;
    }
    Ok(())
}

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(PdfError::DocumentError(
            "Usage: compare_cjk_advance INPUT.pdf OUTPUT.pdf".into(),
        ));
    }
    let doc = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    if doc.try_page_count()? != 2 {
        return Err(PdfError::DocumentError(
            "Expected two-page CJK fixture".into(),
        ));
    }
    let original = doc.get_page(0)?.content_program()?;
    let operation = original
        .operations()
        .iter()
        .enumerate()
        .filter(|(_, op)| op.operator.name() == "Tj")
        .nth(1)
        .map(|(index, _)| index)
        .ok_or_else(|| PdfError::ObjectError("Second Tj missing".into()))?;
    let font_name = original.operations()[..operation]
        .iter()
        .rev()
        .find(|op| op.operator.name() == "Tf")
        .ok_or_else(|| PdfError::FontError("Font missing".into()))?
        .operator
        .get_as_name(0)?
        .name();
    let page = doc.get_page(0)?;
    if original.operations()[operation]
        .operator
        .get_as_string(0)?
        .bytes()
        != page.encode_text_for_font(font_name, "你好，世界！Hello PDF")?
    {
        return Err(PdfError::ObjectError(
            "Expected original CJK sentence".into(),
        ));
    }
    let mut following = ContentBuilder::new();
    following.show_encoded_text(&page.encode_text_for_font(font_name, " PDF")?)?;
    drop(page);
    let split = original.operations()[operation].lexical_range.end;
    let mut source = original.source_bytes()[..split].to_vec();
    source.push(b'\n');
    source.extend_from_slice(following.as_bytes());
    source.extend_from_slice(&original.source_bytes()[split..]);
    doc.replace_page_contents(0, source)?;
    let baseline = doc.get_page(0)?.content_program()?;
    doc.copy_page(0, 2)?;
    doc.copy_page(0, 3)?;
    let snapshot = doc.get_page(2)?.content_program()?;
    doc.replace_page_text_preserving_advance(2, &snapshot, operation, "你好！")?;
    let snapshot = doc.get_page(3)?.content_program()?;
    doc.replace_page_text(3, &snapshot, operation, "你好！")?;
    if doc.get_page(0)?.content_program()?.source_bytes() != baseline.source_bytes() {
        return Err(PdfError::DocumentError("Shared baseline changed".into()));
    }
    check(&doc)?;
    doc.save_to_file(&args[1])?;
    drop(doc);
    let reopened = PdfDocument::open_with_mode(&args[1], ParseMode::Strict)?;
    check(&reopened)
}
