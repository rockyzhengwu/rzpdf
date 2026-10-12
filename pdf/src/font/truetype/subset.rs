//! Static original-GID subset assembly. Input bytes and the existing SFNT /
//! TrueTypeProgram parsers' allocations are outside the output operation pool.
use super::{
    reader::TrueTypeReader,
    sfnt::Sfnt,
    sfnt_writer::write_sfnt,
    subset_name::rewrite_postscript_name,
    subset_tables::{TableAction, table_action},
    tables::loca::GlyphLocations,
    true_type_program::TrueTypeProgram,
};
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    font::cid_encoding::UnicodeCidEncoding,
    parser::options::Limits,
};

pub(crate) struct SubsetFont {
    pub(crate) bytes: BudgetedBytes,
    pub(crate) omitted_tables: Vec<[u8; 4]>,
    pub(crate) _report_lease: MemoryLease,
}

/// Validate direct scalar mappings before re-encoding an original-GID CID font.
/// Borrow ToUnicode mappings; do not allocate a String per decoded character.
/// Multi-scalar/shaped mappings require a separate editing strategy.
pub(crate) fn validate_subset_codes(
    font: &crate::font::pdf_font::Font,
    source: &TrueTypeProgram,
    codes: &[u8],
    limits: Limits,
) -> PdfResult<usize> {
    visit_subset_scalars(font, source, codes, limits, |_, _| Ok(()))
}

/// Visit checked Unicode scalars and original GIDs for repertoire planning.
/// On later failure, callers must discard partial plans; visitor effects are
/// not transactional. This function never mutates the font or document.
pub(crate) fn visit_subset_scalars(
    font: &crate::font::pdf_font::Font,
    source: &TrueTypeProgram,
    codes: &[u8],
    limits: Limits,
    mut visitor: impl FnMut(char, u16) -> PdfResult<()>,
) -> PdfResult<usize> {
    use crate::font::{GlyphDesc, pdf_font::Font};
    if codes.len() > limits.max_decoded_bytes {
        return Err(PdfError::ResourceLimit("subset source code bytes".into()));
    }
    let Font::Type0(cid_font) = font else {
        return Err(PdfError::UnsupportedFeature(
            "Direct subset editing requires a CID font".into(),
        ));
    };
    font.visit_chars_limited(codes, limits.max_text_chars, |code| {
        let mapping = cid_font.unicode_mapping(&code).ok_or_else(|| PdfError::FontError("Subset editing requires an explicit Unicode mapping".into()))?;
        let mut scalars = mapping.chars();
        let scalar = scalars.next().ok_or_else(|| PdfError::FontError("Empty subset Unicode mapping".into()))?;
        if scalars.next().is_some() {
            return Err(PdfError::UnsupportedFeature("Multi-scalar subset editing requires shaping support".into()));
        }
        let gid = source.unicode_glyph_id(scalar)?.ok_or_else(|| PdfError::FontError("Subset source lacks original Unicode character".into()))?;
        if !matches!(font.get_glyph(&code), Some(GlyphDesc::Gid(actual)) if actual == u32::from(gid)) {
            return Err(PdfError::FontError("Subset Unicode and source GID do not match".into()));
        }
        visitor(scalar, gid)
    })
}

/// Decode validated original CID codes for immutable repertoire planning.
/// Two bounded passes avoid growing a retained buffer while unaccounted.
pub(crate) fn decode_subset_codes(
    font: &crate::font::pdf_font::Font,
    source: &TrueTypeProgram,
    codes: &[u8],
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    let mut length = 0usize;
    visit_subset_scalars(font, source, codes, limits, |scalar, _| {
        length = length
            .checked_add(scalar.len_utf8())
            .filter(|&value| value <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("subset decoded Unicode bytes".into()))?;
        Ok(())
    })?;
    let mut lease = budget.reserve(length)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("subset Unicode allocation".into()))?;
    lease.resize(output.capacity())?;
    visit_subset_scalars(font, source, codes, limits, |scalar, _| {
        let mut utf8 = [0u8; 4];
        output.extend_from_slice(scalar.encode_utf8(&mut utf8).as_bytes());
        Ok(())
    })?;
    BudgetedBytes::from_reserved_vec(output, lease)
}

/// Plan one original string without mutating any font or content. Retained
/// prefix/suffix are decoded with the original binding; replacement coverage
/// is checked against the caller's full source before resource registration.
pub(crate) fn plan_subset_string(
    font: &crate::font::pdf_font::Font,
    source: &TrueTypeProgram,
    codes: &[u8],
    range: std::ops::Range<usize>,
    replacement: &str,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    validate_subset_codes(font, source, codes, limits)?;
    let prefix = codes
        .get(..range.start)
        .ok_or_else(|| PdfError::ObjectError("Subset string range out of bounds".into()))?;
    let suffix = codes
        .get(range.end..)
        .filter(|_| range.start <= range.end)
        .ok_or_else(|| {
            PdfError::ObjectError("Subset string range reversed or out of bounds".into())
        })?;
    // Decode each slice separately: a split multi-byte character must reject.
    let prefix = decode_subset_codes(font, source, prefix, limits, budget)?;
    let suffix = decode_subset_codes(font, source, suffix, limits, budget)?;
    let replacement_count = crate::font::pdf_font::encoding_character_count(replacement, limits)?;
    for scalar in replacement.chars() {
        if source.unicode_glyph_id(scalar)?.is_none() {
            return Err(PdfError::FontError(
                "Subset source lacks replacement character".into(),
            ));
        }
    }
    let prefix_text = std::str::from_utf8(prefix.as_slice())
        .map_err(|_| PdfError::FontError("Invalid planned Unicode".into()))?;
    let suffix_text = std::str::from_utf8(suffix.as_slice())
        .map_err(|_| PdfError::FontError("Invalid planned Unicode".into()))?;
    prefix_text
        .chars()
        .count()
        .checked_add(replacement_count)
        .and_then(|count| count.checked_add(suffix_text.chars().count()))
        .filter(|&count| count <= limits.max_text_chars)
        .ok_or_else(|| PdfError::ResourceLimit("subset planned characters".into()))?;
    let length = prefix
        .as_slice()
        .len()
        .checked_add(replacement.len())
        .and_then(|length| length.checked_add(suffix.as_slice().len()))
        .filter(|&length| length <= limits.max_decoded_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("subset planned Unicode bytes".into()))?;
    let mut lease = budget.reserve(length)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("subset Unicode plan allocation".into()))?;
    lease.resize(output.capacity())?;
    output.extend_from_slice(prefix.as_slice());
    output.extend_from_slice(replacement.as_bytes());
    output.extend_from_slice(suffix.as_slice());
    BudgetedBytes::from_reserved_vec(output, lease)
}

/// Plan a whole TJ repertoire across string endpoints; numeric slots are not
/// characters. This does not publish content or decide compensation placement.
pub(crate) fn plan_subset_text_array_range(
    font: &crate::font::pdf_font::Font,
    source: &TrueTypeProgram,
    array: &crate::objects::pdf_array::PdfArray,
    range: std::ops::Range<(usize, usize)>,
    replacement: &str,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    if array.len() > limits.max_objects {
        return Err(PdfError::ResourceLimit("subset TJ items".into()));
    }
    if range.start > range.end {
        return Err(PdfError::ObjectError("Subset TJ range reversed".into()));
    }
    for (index, offset) in [range.start, range.end] {
        let string = array
            .get(index)
            .and_then(|item| item.as_string())
            .ok_or_else(|| PdfError::ObjectError("Subset TJ endpoint requires a string".into()))?;
        if offset > string.bytes().len() {
            return Err(PdfError::ObjectError(
                "Subset TJ endpoint out of bounds".into(),
            ));
        }
    }
    crate::font::pdf_font::encoding_character_count(replacement, limits)?;
    for scalar in replacement.chars() {
        if source.unicode_glyph_id(scalar)?.is_none() {
            return Err(PdfError::FontError(
                "Subset source lacks replacement character".into(),
            ));
        }
    }
    // Two bounded borrowed traversals; no per-item decoded Strings or metadata
    // vectors. Every original string is validated, even if entirely removed.
    let traverse = |emit: &mut dyn FnMut(char) -> PdfResult<()>| -> PdfResult<()> {
        let mut input_chars = 0usize;
        let mut input_bytes = 0usize;
        for (index, item) in array.into_iter().enumerate() {
            let Some(string) = item.as_string() else {
                if item.as_number().is_none() {
                    return Err(PdfError::ObjectError(
                        "TJ item must be string or number".into(),
                    ));
                }
                continue;
            };
            let codes = string.bytes();
            input_bytes = input_bytes
                .checked_add(codes.len())
                .filter(|&size| size <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("subset TJ input bytes".into()))?;
            input_chars = input_chars
                .checked_add(validate_subset_codes(font, source, codes, limits)?)
                .filter(|&count| count <= limits.max_text_chars)
                .ok_or_else(|| PdfError::ResourceLimit("subset TJ input characters".into()))?;
            if index < range.start.0 || index > range.end.0 {
                visit_subset_scalars(font, source, codes, limits, |scalar, _| emit(scalar))?;
                continue;
            }
            if index == range.start.0 {
                visit_subset_scalars(
                    font,
                    source,
                    &codes[..range.start.1],
                    limits,
                    |scalar, _| emit(scalar),
                )?;
                for scalar in replacement.chars() {
                    emit(scalar)?;
                }
            }
            if index == range.end.0 {
                visit_subset_scalars(font, source, &codes[range.end.1..], limits, |scalar, _| {
                    emit(scalar)
                })?;
            }
        }
        Ok(())
    };
    let mut length = 0usize;
    let mut count = 0usize;
    traverse(&mut |scalar| {
        count = count
            .checked_add(1)
            .filter(|&count| count <= limits.max_text_chars)
            .ok_or_else(|| PdfError::ResourceLimit("subset TJ planned characters".into()))?;
        length = length
            .checked_add(scalar.len_utf8())
            .filter(|&length| length <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("subset TJ planned Unicode bytes".into()))?;
        Ok(())
    })?;
    let mut lease = budget.reserve(length)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("subset TJ plan allocation".into()))?;
    lease.resize(output.capacity())?;
    traverse(&mut |scalar| {
        let mut utf8 = [0u8; 4];
        output.extend_from_slice(scalar.encode_utf8(&mut utf8).as_bytes());
        Ok(())
    })?;
    BudgetedBytes::from_reserved_vec(output, lease)
}

/// Check source compatibility with this engine's static original-GID subset
/// profile. Not a name-based identity check or support for renumbered subsets.
/// Removed outlines cannot prove identity of the missing source glyphs.
/// Existing SFNT/parser allocations remain outside the operation pool.
pub(crate) fn validate_subset_source(
    embedded: &[u8],
    source: &[u8],
    limits: Limits,
) -> PdfResult<()> {
    let mismatch = || PdfError::FontError("Subset source does not match embedded font".into());
    if embedded.get(..4) != Some(&[0, 1, 0, 0]) || source.get(..4) != Some(&[0, 1, 0, 0]) {
        return Err(mismatch());
    }
    let old = Sfnt::try_new_with_limits(&mut TrueTypeReader::new(embedded), limits)?;
    let full = Sfnt::try_new_with_limits(&mut TrueTypeReader::new(source), limits)?;
    fn bytes<'a>(font: &'a [u8], directory: &Sfnt, tag: &str) -> PdfResult<&'a [u8]> {
        let mismatch = || PdfError::FontError("Subset source does not match embedded font".into());
        let record = directory.get_table(tag).ok_or_else(mismatch)?;
        let start = record.offset() as usize;
        let end = start
            .checked_add(record.length() as usize)
            .ok_or_else(mismatch)?;
        font.get(start..end).ok_or_else(mismatch)
    }
    let mut compared = 0usize;
    let mut charge = |length: usize| -> PdfResult<()> {
        compared = compared
            .checked_add(length)
            .filter(|&value| value <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("subset source comparison bytes".into()))?;
        Ok(())
    };
    // Compare both directories so a missing optional preserved table also fails.
    for record in old.table_records().chain(full.table_records()) {
        if table_action(record.tag(), true)? == TableAction::Preserve {
            let embedded_table = bytes(embedded, &old, record.tag())?;
            let source_table = bytes(source, &full, record.tag())?;
            charge(embedded_table.len().max(source_table.len()))?;
            if embedded_table != source_table {
                return Err(mismatch());
            }
        }
    }
    let old_head = bytes(embedded, &old, "head")?;
    let full_head = bytes(source, &full, "head")?;
    if old_head.len() < 54 || full_head.len() < 54 {
        return Err(mismatch());
    }
    // Only adjustment, global bounds and loca format are rewritten by our
    // subset builder. Flags, timestamps, units, style and other fields stay put.
    for range in [0..8, 12..36, 44..50, 52..54] {
        charge(range.len())?;
        if old_head[range.clone()] != full_head[range] {
            return Err(mismatch());
        }
    }
    let old_program = TrueTypeProgram::try_new(embedded)?;
    let full_program = TrueTypeProgram::try_new(source)?;
    if old_program
        .horizontal_metrics()
        .ok_or_else(mismatch)?
        .units_per_em()
        != full_program
            .horizontal_metrics()
            .ok_or_else(mismatch)?
            .units_per_em()
    {
        return Err(mismatch());
    }
    let count = old_program.num_glyphs().ok_or_else(mismatch)?;
    if count != full_program.num_glyphs().ok_or_else(mismatch)?
        || usize::from(count) > limits.max_objects
    {
        return Err(mismatch());
    }
    let old_locations = TrueTypeProgram::checked_glyph_locations(embedded, limits)?;
    let full_locations = TrueTypeProgram::checked_glyph_locations(source, limits)?;
    for gid in 0..count {
        let actual = old_locations.glyph_data(gid)?;
        if actual.is_empty() {
            continue;
        }
        let original = full_locations.glyph_data(gid)?;
        charge(actual.len().max(original.len()))?;
        if !actual.starts_with(original)
            || actual.len() - original.len() > 3
            || actual[original.len()..].iter().any(|byte| *byte != 0)
        {
            return Err(mismatch());
        }
    }
    Ok(())
}

pub(crate) fn build_subset(
    source: &[u8],
    encoding: &UnicodeCidEncoding,
    postscript_name: &str,
    omit_layout: bool,
    limits: &Limits,
    budget: &OperationBudget,
) -> PdfResult<SubsetFont> {
    if source.get(..4) != Some(&[0, 1, 0, 0][..]) {
        return Err(PdfError::FontError(
            "Subset requires standalone static TrueType".into(),
        ));
    }
    let sfnt = Sfnt::try_new_with_limits(&mut TrueTypeReader::new(source), *limits)?;
    // Decide all table policies before allocating any rebuilt font tables.
    for record in sfnt.table_records() {
        table_action(record.tag(), omit_layout)?;
    }
    let table = |tag: &str| -> PdfResult<&[u8]> {
        let record = sfnt
            .get_table(tag)
            .ok_or_else(|| PdfError::FontError(format!("Missing subset table: {tag}")))?;
        let start = record.offset() as usize;
        let end = start
            .checked_add(record.length() as usize)
            .ok_or_else(|| PdfError::FontError("Subset source table range overflow".into()))?;
        source
            .get(start..end)
            .ok_or_else(|| PdfError::FontError("Subset source table exceeds input".into()))
    };
    for tag in [
        "head", "hhea", "hmtx", "maxp", "cmap", "name", "OS/2", "post", "glyf", "loca",
    ] {
        table(tag)?;
    }
    if table("maxp")?.get(..4) != Some(&[0, 1, 0, 0][..]) {
        return Err(PdfError::FontError(
            "TrueType subset requires maxp version 1".into(),
        ));
    }
    let program = TrueTypeProgram::try_new_with_limits(source, *limits)?;
    program.validate_outline_embedding(true, true)?;
    if sfnt.get_table("vhea").is_some() || sfnt.get_table("vmtx").is_some() {
        let vertical = table("vhea")?;
        let metrics = table("vmtx")?;
        let glyphs = program
                .num_glyphs()
                .ok_or_else(|| PdfError::FontError("Missing subset glyph count".into()))?;
        super::tables::vertical::VerticalMetrics::try_new(vertical, metrics, glyphs)?;
    }
    for (_, character, gid, width) in encoding.glyph_mappings() {
        if program.unicode_glyph_id(character)? != Some(gid)
            || program
                .horizontal_metrics()
                .ok_or_else(|| PdfError::FontError("Missing subset metrics".into()))?
                .advance_width_1000(gid)?
                != width
        {
            return Err(PdfError::FontError(
                "Subset encoding does not match source font".into(),
            ));
        }
    }
    let head = table("head")?;
    let format = head
        .get(50..52)
        .ok_or_else(|| PdfError::FontError("Truncated subset head".into()))?;
    let locations = GlyphLocations::try_new(
        table("loca")?,
        table("glyf")?,
        program
            .num_glyphs()
            .ok_or_else(|| PdfError::FontError("Missing subset glyph count".into()))?,
        i16::from_be_bytes([format[0], format[1]]),
    )?;
    let closure = locations.subset_closure(
        encoding.glyph_mappings().map(|(_, _, gid, _)| gid),
        limits,
        budget,
    )?;
    let (glyf, loca) = closure.rebuild_glyf_loca(limits, budget)?;
    let cmap = encoding.subset_cmap(budget)?;
    let names = rewrite_postscript_name(table("name")?, postscript_name, limits, budget)?;
    let (mut head_copy, head_lease) =
        BudgetedBytes::copy_from_slice(budget, head)?.into_reserved_parts();
    head_copy[50..52].copy_from_slice(&1i16.to_be_bytes());
    for (slot, value) in head_copy[36..44].chunks_exact_mut(2).zip(closure.bounds()) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
    let rebuilt_head = BudgetedBytes::from_reserved_vec(head_copy, head_lease)?;
    let count = sfnt.table_records().len();
    let mut directory_lease = budget.reserve(count * std::mem::size_of::<([u8; 4], &[u8])>())?;
    let mut tables = Vec::new();
    tables
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("Subset table list allocation".into()))?;
    directory_lease.resize(tables.capacity() * std::mem::size_of::<([u8; 4], &[u8])>())?;
    let mut report_lease = budget.reserve(count * 4)?;
    let mut omitted = Vec::new();
    omitted
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("Subset omission report allocation".into()))?;
    report_lease.resize(omitted.capacity() * 4)?;
    for record in sfnt.table_records() {
        let tag: [u8; 4] = record
            .tag()
            .as_bytes()
            .try_into()
            .map_err(|_| PdfError::FontError("Invalid subset table tag".into()))?;
        let data = match table_action(record.tag(), omit_layout)? {
            TableAction::Omit => {
                omitted.push(tag);
                continue;
            }
            TableAction::Preserve => table(record.tag())?,
            TableAction::Rebuild => match record.tag() {
                "head" => rebuilt_head.as_slice(),
                "glyf" => glyf.as_slice(),
                "loca" => loca.as_slice(),
                "cmap" => cmap.as_slice(),
                "name" => names.as_slice(),
                _ => return Err(PdfError::FontError("Missing subset table rebuilder".into())),
            },
        };
        tables.push((tag, data));
    }
    omitted.sort_unstable();
    let bytes = write_sfnt(&tables, limits, budget)?;
    Ok(SubsetFont {
        bytes,
        omitted_tables: omitted,
        _report_lease: report_lease,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn automatic_subset_cross_call_is_atomic_and_keeps_unselected_invocations() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                resource_manager::ResourceKind, subset_text_edit::SubsetTextEditOptions},
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::source_provenance::NestedTextCodePosition,
            parser::syntax::SyntaxParser,
        };
        for (preserve, replacement, nested) in [(false, "BB"), (true, "BB"), (false, ""), (true, "")]
            .into_iter().flat_map(|(preserve, replacement)|
                [false, true].map(|nested| (preserve, replacement, nested))) {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(), cap_height: 60.0, stem_v: 80.0, serif: false,
            };
            let font = doc.resources().embed_truetype_subset_font(
                0, &source, "A", embedding(), *b"AAAAAA", false, &pool,
            ).unwrap();
            let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".as_slice(),
            )).unwrap()).get_object().unwrap().into_dict().unwrap();
            dictionary.insert("Resources".into(), PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()));
            let object = PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary,
                format!("BT /{} 12 Tf 1 Tc 20 50 Td <00010001> Tj <0001> Tj ET", font.font.resource_name).into_bytes(),
            ).unwrap());
            let original = object.clone();
            let handle = doc.try_insert_object(object).unwrap();
            let mut parent_original = None;
            let invocation = if nested {
                let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                    b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".as_slice(),
                )).unwrap()).get_object().unwrap().into_dict().unwrap();
                let resources = crate::objects::pdf_dict::PdfDict::new(std::collections::HashMap::from([
                    ("XObject".into(), PdfObject::PdfDict(crate::objects::pdf_dict::PdfDict::new(
                        std::collections::HashMap::from([("Leaf".into(), PdfObject::PdfReference(
                            crate::objects::pdf_reference::PdfReference::from_id(handle),
                        ))]),
                    ))),
                ]));
                dictionary.insert("Resources".into(), PdfObject::PdfDict(resources));
                let parent = PdfObject::PdfStream(PdfStream::from_decoded_data(
                    dictionary, b"q /Leaf Do Q".to_vec(),
                ).unwrap());
                let parent_handle = doc.try_insert_object(parent.clone()).unwrap();
                parent_original = Some((parent_handle, parent));
                parent_handle
            } else { handle };
            for page in 0..2 {
                let name = doc.resources().register_existing(page, ResourceKind::Form, invocation).unwrap();
                doc.replace_page_contents(page, format!("/{name} Do /{name} Do /{name} Do").into_bytes()).unwrap();
            }
            let saved = doc.save_to_bytes().unwrap();
            let baseline = pool.live_bytes();
            let options = || SubsetTextEditOptions {
                source: &source, embedding: embedding(), subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false, preserve_advance: preserve,
            };
            let attempt = |budget: &OperationBudget| {
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let report = doc.get_page(0).unwrap().extract_characters_paint_order_report().unwrap();
                doc.replace_nested_text_span_with_subset_and_budget(0, &root,
                    NestedTextCodePosition { source: report.characters[1].nested_source.as_ref().unwrap(), array_item: None, offset: 2 }
                    ..NestedTextCodePosition { source: report.characters[3].nested_source.as_ref().unwrap(), array_item: None, offset: 2 },
                    replacement, options(), budget)
            };
            // CID codes are two bytes: neither endpoint may split a code or
            // extend past its original string. Reject without publishing fonts.
            for (start, end) in [(1, 2), (3, 2), (6, 2), (2, 1), (2, 3), (2, 6)] {
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let report = doc.get_page(0).unwrap().extract_characters_paint_order_report().unwrap();
                let result = doc.replace_nested_text_span_with_subset_and_budget(
                    0, &root,
                    NestedTextCodePosition {
                        source: report.characters[1].nested_source.as_ref().unwrap(),
                        array_item: None, offset: start,
                    }..NestedTextCodePosition {
                        source: report.characters[3].nested_source.as_ref().unwrap(),
                        array_item: None, offset: end,
                    },
                    replacement, options(), &pool,
                );
                assert!(result.is_err(), "invalid CID endpoints {start}..{end}");
                assert_eq!(doc.save_to_bytes().unwrap(), saved);
                assert_eq!(pool.live_bytes(), baseline);
            }
            let tiny = OperationBudget::new(1);
            assert!(matches!(attempt(&tiny), Err(PdfError::ResourceLimit(_))));
            assert_eq!(tiny.live_bytes(), 0);
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            let error = doc.edit_transaction(|_| {
                attempt(&pool)?;
                Err::<(), _>(PdfError::ObjectError("after cross-call subset publication".into()))
            });
            assert!(matches!(error, Err(PdfError::ObjectError(message)) if message == "after cross-call subset publication"));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = doc.edit_transaction(|_| -> crate::error::PdfResult<()> {
                    attempt(&pool)?;
                    panic!("after cross-call subset publication");
                });
            }));
            assert_eq!(caught.unwrap_err().downcast_ref::<&str>().copied(), Some("after cross-call subset publication"));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let before = doc.get_page(0).unwrap().extract_characters_paint_order().unwrap();
            attempt(&pool).unwrap();
            assert_eq!(doc.ctx().resolve_owned(&PdfObject::PdfReference(crate::objects::pdf_reference::PdfReference::from_id(handle))).unwrap(), original);
            if let Some((handle, original)) = parent_original {
                assert_eq!(doc.ctx().resolve_owned(&PdfObject::PdfReference(
                    crate::objects::pdf_reference::PdfReference::from_id(handle),
                )).unwrap(), original);
            }
            let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), crate::parser::syntax::ParseMode::Strict).unwrap();
            let after = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap();
            assert_eq!(after.iter().filter_map(|c| c.unicode.as_deref()).collect::<String>(),
                if replacement.is_empty() { "AAAAAA" } else { "ABBAAAAA" });
            assert_eq!(reopened.get_page(1).unwrap().extract_characters_paint_order().unwrap().iter().filter_map(|c| c.unicode.as_deref()).collect::<String>(), "AAAAAAAAA");
            let removed_replacement_chars = if replacement.is_empty() { 2 } else { 0 };
            for (old, new) in [(4, 3), (5, 4)] {
                let new = new - removed_replacement_chars;
                let delta = (before[old].position.x() - after[new].position.x()).abs();
                assert!(if preserve { delta < 0.002 } else { delta > 1.0 });
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
            for (old, new) in [(6, 5), (7, 6), (8, 7)] {
                let new = new - removed_replacement_chars;
                assert!((before[old].position.x() - after[new].position.x()).abs() < 0.002);
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
            drop(font);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_cross_call_character_limits_are_aggregate_and_atomic() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                resource_manager::ResourceKind, subset_text_edit::SubsetTextEditOptions},
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::{content_program::ContentProgram, source_provenance::{
                FormInvocation, NestedDrawSource, NestedTextCodePosition}},
            parser::{options::ParseOptions, syntax::{SyntaxParser, ParseMode}},
        };
        for (limit, end_offset, replacement, expected_error) in [
            (3, 4, "BB", Some("span aggregate input characters")),
            (4, 4, "BB", None),
            (4, 0, "BBB", Some("span aggregate output characters")),
            (5, 0, "BBB", None),
        ] {
            let source = fixture();
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits { max_text_chars: limit, ..Limits::default() },
                ..ParseOptions::default()
            }).unwrap();
            doc.add_page(0).unwrap();
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(), cap_height: 60.0,
                stem_v: 80.0, serif: false,
            };
            let font = doc.resources().embed_truetype_subset_font(
                0, &source, "A", embedding(), *b"AAAAAA", false, &pool,
            ).unwrap();
            let leaf_bytes = format!("BT /{} 12 Tf <00010001> Tj ET", font.font.resource_name).into_bytes();
            let leaf = ContentProgram::from_decoded_bytes(leaf_bytes.clone(), ParseOptions::default()).unwrap();
            let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".as_slice(),
            )).unwrap()).get_object().unwrap().into_dict().unwrap();
            dictionary.insert("Resources".into(), PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()));
            let original = PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary, leaf_bytes).unwrap());
            let handle = doc.try_insert_object(original.clone()).unwrap();
            let name = doc.resources().register_existing(0, ResourceKind::Form, handle).unwrap();
            doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes()).unwrap();
            let saved = doc.save_to_bytes().unwrap();
            let baseline = pool.live_bytes();
            let attempt = |end_call: usize, end: usize, text: &str| {
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                // Construct descriptors from the actual current programs:
                // extraction itself would exceed the restricted page limit.
                let descriptor = |call: usize| NestedDrawSource {
                    invocations: vec![FormInvocation {
                        form_id: Some(handle), operation_index: call,
                        operator_byte_offset: root.operations()[call].operator.byte_offset(),
                    }],
                    operation_index: 2,
                    operator_byte_offset: leaf.operations()[2].operator.byte_offset(),
                    geometry_operation_indices: Vec::new(),
                };
                let start = descriptor(0);
                let finish = descriptor(end_call);
                doc.replace_nested_text_span_with_subset_and_budget(0, &root,
                    NestedTextCodePosition { source: &start, array_item: None, offset: 0 }
                    ..NestedTextCodePosition { source: &finish, array_item: None, offset: end },
                    text, SubsetTextEditOptions {
                        source: &source, embedding: embedding(), subset_tag: *b"BBBBBB",
                        omit_layout_and_auxiliary: false, preserve_advance: true,
                    }, &pool)
            };
            let result = attempt(1, end_offset, replacement);
            let expected = if let Some(message) = expected_error {
                assert!(matches!(result, Err(PdfError::ResourceLimit(ref actual)) if actual == message),
                    "limit {limit}, end {end_offset}: {result:?}");
                assert_eq!(doc.save_to_bytes().unwrap(), saved);
                assert_eq!(pool.live_bytes(), baseline);
                // A narrower selection fits the same limit after rollback;
                // attempt obtains a fresh root snapshot, not a stale retry.
                attempt(0, 4, "B").unwrap();
                "BAA"
            } else {
                result.unwrap();
                if end_offset == 4 { "BB" } else { "BBBAA" }
            };
            assert_eq!(doc.resolve_object(handle).unwrap(), original);
            assert_eq!(pool.live_bytes(), baseline);
            let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
            let text = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap()
                .iter().filter_map(|ch| ch.unicode.as_deref()).collect::<String>();
            assert_eq!(text, expected);
            drop(font);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_cross_call_decoded_work_counts_repeated_forms() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                resource_manager::ResourceKind, subset_text_edit::SubsetTextEditOptions},
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::{content_program::ContentProgram, source_provenance::{
                FormInvocation, NestedDrawSource, NestedTextCodePosition}},
            parser::{options::ParseOptions, syntax::{SyntaxParser, ParseMode}},
        };
        let source = fixture();
        let initial = PdfDocument::new().unwrap();
        initial.add_page(0).unwrap();
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let embedding = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(), cap_height: 60.0, stem_v: 80.0, serif: false,
        };
        let font = initial.resources().embed_truetype_subset_font(
            0, &source, "A", embedding(), *b"AAAAAA", false, &pool,
        ).unwrap();
        // Padding makes expanded work larger than the font tables and the
        // predefined Identity-H CMap source (also charged against this limit).
        // The third
        // unselected call leaves headroom for the edited two-node publication.
        let leaf_bytes = format!("BT /{} 12 Tf <00010001> Tj ET\n%{}\n",
            font.font.resource_name, "padding".repeat(2000)).into_bytes();
        let leaf = ContentProgram::from_decoded_bytes(leaf_bytes.clone(), ParseOptions::default()).unwrap();
        let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
            b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".as_slice(),
        )).unwrap()).get_object().unwrap().into_dict().unwrap();
        dictionary.insert("Resources".into(), PdfObject::PdfDict(initial.get_page(0).unwrap().resource().unwrap()));
        let original = PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary, leaf_bytes.clone()).unwrap());
        let handle = initial.try_insert_object(original.clone()).unwrap();
        let name = initial.resources().register_existing(0, ResourceKind::Form, handle).unwrap();
        let root_bytes = format!("/{name} Do /{name} Do /{name} Do").into_bytes();
        initial.replace_page_contents(0, root_bytes.clone()).unwrap();
        let saved = initial.save_to_bytes().unwrap();
        drop(font);
        assert_eq!(pool.live_bytes(), 0);
        let exact = root_bytes.len() + 3 * leaf_bytes.len();
        for limit in [exact - 1, exact] {
            let doc = PdfDocument::from_bytes_with_options(saved.clone(), ParseOptions {
                limits: Limits { max_decoded_bytes: limit, ..Limits::default() },
                ..ParseOptions::default()
            }).unwrap();
            let before = doc.save_to_bytes().unwrap();
            let attempt = || {
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let descriptor = |call: usize| NestedDrawSource {
                    invocations: vec![FormInvocation {
                        form_id: Some(handle), operation_index: call,
                        operator_byte_offset: root.operations()[call].operator.byte_offset(),
                    }],
                    operation_index: 2,
                    operator_byte_offset: leaf.operations()[2].operator.byte_offset(),
                    geometry_operation_indices: Vec::new(),
                };
                let start = descriptor(0);
                let end = descriptor(1);
                doc.replace_nested_text_span_with_subset_and_budget(0, &root,
                    NestedTextCodePosition { source: &start, array_item: None, offset: 0 }
                    ..NestedTextCodePosition { source: &end, array_item: None, offset: 4 },
                    "BB", SubsetTextEditOptions {
                        source: &source, embedding: embedding(), subset_tag: *b"BBBBBB",
                        omit_layout_and_auxiliary: false, preserve_advance: true,
                    }, &pool)
            };
            let result = attempt();
            if limit < exact {
                assert!(matches!(result, Err(PdfError::ResourceLimit(ref message))
                    if message == "span expanded decoded work"), "{result:?}");
                assert_eq!(doc.save_to_bytes().unwrap(), before);
                assert_eq!(pool.live_bytes(), 0);
                // Expanded work covers all calls even for a narrow selection.
                // Explicitly remove the third call, then acquire a fresh root.
                doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes()).unwrap();
                attempt().unwrap();
            } else {
                result.unwrap();
            }
            assert_eq!(doc.resolve_object(handle).unwrap(), original);
            assert_eq!(pool.live_bytes(), 0);
            let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
            let text = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap()
                .iter().filter_map(|ch| ch.unicode.as_deref()).collect::<String>();
            assert_eq!(text, if limit < exact { "BB" } else { "BBAA" });
        }
    }

    #[test]
    fn automatic_subset_span_inserts_in_root_and_deletes_in_form() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                resource_manager::ResourceKind, subset_text_edit::SubsetTextEditOptions},
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::source_provenance::NestedTextCodePosition,
            parser::syntax::{SyntaxParser, ParseMode},
        };
        for (preserve, mixed, reverse) in [
            (false, false, false), (true, false, false),
            (false, true, false), (true, true, false),
            (false, false, true), (true, false, true),
        ] {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(), cap_height: 60.0, stem_v: 80.0, serif: false,
            };
            let font = doc.resources().embed_truetype_subset_font(
                0, &source, "A", embedding(), *b"AAAAAA", false, &pool,
            ).unwrap();
            let leaf_content = if mixed {
                let name = doc.resources().register_standard_font(
                    0, crate::document::resource_manager::StandardFont::Courier,
                ).unwrap();
                format!("BT /{name} 18 Tf 3 Tc 20 50 Td (AAA) Tj ET")
            } else {
                format!("BT /{} 12 Tf 1 Tc 20 50 Td <000100010001> Tj ET", font.font.resource_name)
            };
            let mut dictionary = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".as_slice(),
            )).unwrap()).get_object().unwrap().into_dict().unwrap();
            dictionary.insert("Resources".into(), PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()));
            let original = PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary,
                leaf_content.into_bytes(),
            ).unwrap());
            let handle = doc.try_insert_object(original.clone()).unwrap();
            for page in 0..2 {
                let font_name = if page == 0 { font.font.resource_name.clone() }
                    else { font.font.register_on_page(&doc, page).unwrap() };
                let form_name = doc.resources().register_existing(page, ResourceKind::Form, handle).unwrap();
                let root_text = format!("BT /{font_name} 12 Tf 1 Tc 20 80 Td <00010001> Tj ET");
                let content = if reverse {
                    format!("/{form_name} Do {root_text} /{form_name} Do")
                } else { format!("{root_text} /{form_name} Do /{form_name} Do") };
                doc.replace_page_contents(page, content.into_bytes()).unwrap();
            }
            let saved = doc.save_to_bytes().unwrap();
            let baseline = pool.live_bytes();
            let attempt = |budget: &OperationBudget| {
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let report = doc.get_page(0).unwrap().extract_characters_paint_order_report().unwrap();
                let page_source = crate::page::source_provenance::NestedDrawSource::from_page_text(
                    report.characters[if reverse { 3 } else { 0 }].source.as_ref().unwrap(),
                );
                let (start, end) = if reverse {
                    (report.characters[0].nested_source.as_ref().unwrap(), &page_source)
                } else { (&page_source, report.characters[2].nested_source.as_ref().unwrap()) };
                doc.replace_nested_text_span_with_subset_and_budget(0, &root,
                    NestedTextCodePosition { source: start, array_item: None, offset: 2 }
                    ..NestedTextCodePosition { source: end, array_item: None, offset: if mixed { 1 } else { 2 } },
                    "BB", SubsetTextEditOptions { source: &source, embedding: embedding(),
                        subset_tag: *b"BBBBBB", omit_layout_and_auxiliary: false, preserve_advance: preserve }, budget)
            };
            let tiny = OperationBudget::new(1);
            assert!(matches!(attempt(&tiny), Err(PdfError::ResourceLimit(_))));
            assert_eq!(tiny.live_bytes(), 0);
            {
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let report = doc.get_page(0).unwrap().extract_characters_paint_order_report().unwrap();
                let mut page_source = crate::page::source_provenance::NestedDrawSource::from_page_text(
                    report.characters[if reverse { 3 } else { 0 }].source.as_ref().unwrap(),
                );
                page_source.operator_byte_offset = page_source.operator_byte_offset.map(|offset| offset + 1);
                let result = doc.replace_nested_text_span_with_subset_and_budget(0, &root,
                    NestedTextCodePosition { source: &page_source, array_item: None, offset: 2 }
                    ..NestedTextCodePosition { source: report.characters[if reverse { 0 } else { 2 }].nested_source.as_ref().unwrap(), array_item: None, offset: if mixed { 1 } else { 2 } },
                    "BB", SubsetTextEditOptions { source: &source, embedding: embedding(),
                        subset_tag: *b"BBBBBB", omit_layout_and_auxiliary: false, preserve_advance: preserve }, &pool);
                assert!(matches!(result, Err(PdfError::ObjectError(message)) if message == "span endpoint byte position mismatch"));
                assert_eq!(doc.save_to_bytes().unwrap(), saved);
                assert_eq!(pool.live_bytes(), baseline);
            }
            let result = doc.edit_transaction(|_| {
                attempt(&pool)?;
                Err::<(), _>(PdfError::ObjectError("after root subset publication".into()))
            });
            assert!(matches!(result, Err(PdfError::ObjectError(message)) if message == "after root subset publication"));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = doc.edit_transaction(|_| -> crate::error::PdfResult<()> {
                    attempt(&pool)?;
                    panic!("after root subset publication");
                });
            }));
            assert_eq!(caught.unwrap_err().downcast_ref::<&str>().copied(), Some("after root subset publication"));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let before = doc.get_page(0).unwrap().extract_characters_paint_order().unwrap();
            attempt(&pool).unwrap();
            assert_eq!(doc.resolve_object(handle).unwrap(), original);
            let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
            let after = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap();
            assert_eq!(after.iter().filter_map(|c| c.unicode.as_deref()).collect::<String>(),
                if reverse { "ABBAAAA" } else { "ABBAAAAA" });
            assert_eq!(reopened.get_page(1).unwrap().extract_characters_paint_order().unwrap()
                .iter().filter_map(|c| c.unicode.as_deref()).collect::<String>(), "AAAAAAAA");
            let suffix_pairs: &[(usize, usize)] = if reverse { &[(4, 3)] } else { &[(3, 3), (4, 4)] };
            for &(old, new) in suffix_pairs {
                assert_eq!(after[new].font_name, before[old].font_name);
                if mixed { assert_eq!(after[new].font_name.as_deref(), Some("Courier")); }
                let delta = (before[old].position.x() - after[new].position.x()).abs();
                assert!(if preserve { delta < 0.002 } else { delta > 1.0 });
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
            for index in [0, 5, 6, 7] {
                let new = if reverse && index > 0 { index - 1 } else { index };
                assert!((before[index].position.x() - after[new].position.x()).abs() < 0.002);
                assert!((before[index].position.y() - after[new].position.y()).abs() < 0.002);
            }
            drop(font);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_nested_show_isolates_call_and_shared_page() {
        use crate::{
            document::{
                PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                resource_manager::ResourceKind, subset_text_edit::SubsetTextEditOptions,
            },
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_reference::PdfReference, pdf_stream::PdfStream},
            page::content_program::TextCodePosition,
            parser::syntax::SyntaxParser,
        };
        for (preserve, event, inherited, collision, cross_event, mixed) in [
            (false, "Tj"),
            (true, "Tj"),
            (false, "'"),
            (true, "'"),
            (false, "\""),
            (true, "\""),
            (false, "TJ"),
            (true, "TJ"),
        ]
        .into_iter()
        .flat_map(|(preserve, event)| {
            [(false, true), (true, true), (true, false)]
                .map(|(inherited, collision)| (preserve, event, inherited, collision))
        })
        .flat_map(|(preserve, event, inherited, collision)| {
            [(false, false), (true, false), (true, true)].map(|(cross_event, mixed)| {
                (preserve, event, inherited, collision, cross_event, mixed)
            })
        })
        {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(),
                cap_height: 60.0,
                stem_v: 80.0,
                serif: false,
            };
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let font = doc
                .resources()
                .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &pool)
                .unwrap();
            let tail_font = if mixed {
                assert_eq!(
                    doc.resources().register_existing(
                        1,
                        ResourceKind::Font,
                        font.font.object_handle().object_id(),
                    ).unwrap(),
                    font.font.resource_name
                );
                let name = doc.resources().register_standard_font(
                    0, crate::document::resource_manager::StandardFont::Courier,
                ).unwrap();
                assert_eq!(name, doc.resources().register_standard_font(
                    1, crate::document::resource_manager::StandardFont::Courier,
                ).unwrap());
                Some(name)
            } else {
                None
            };
            let mut dictionary = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(
                    b"<< /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let parent_dictionary = dictionary.clone();
            let mut local_resources = doc.get_page(0).unwrap().resource().unwrap();
            let mut local_fonts = doc
                .ctx()
                .resolve_owned(local_resources.get("Font").unwrap())
                .unwrap()
                .into_dict()
                .unwrap();
            let reserved_font = local_fonts.get(&font.font.resource_name).unwrap().clone();
            if collision {
                local_fonts.insert("FontEdit1".into(), reserved_font.clone());
            }
            local_resources.insert("Font".into(), PdfObject::PdfDict(local_fonts));
            // Text state inherits from the caller, but absent Form Resources
            // fall back to page resources, not the caller's resource dictionary.
            // Use explicit leaf resources to exercise a local name collision.
            if !inherited || collision {
                dictionary.insert(
                    "Resources".into(),
                    PdfObject::PdfDict(local_resources.clone()),
                );
            }
            let show = match event {
                "TJ" => "[<0001> -125 % retain\n <0001> +75] TJ",
                "'" => "<00010001> '",
                "\"" => "3 2 <00010001> \"",
                _ => "<00010001> Tj",
            };
            let prefix = if inherited {
                "BT 20 50 Td".to_string()
            } else {
                format!("BT /{} 12 Tf 1 Tc 14 TL 20 50 Td", font.font.resource_name)
            };
            let tail = if let Some(name) = tail_font {
                format!("/{name} 18 Tf 3 Tc (AA) Tj")
            } else {
                "<00010001> Tj".to_string()
            };
            let object = PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    dictionary,
                    format!("{prefix} {show} {tail} ET").into_bytes(),
                )
                .unwrap(),
            );
            let original = object.clone();
            let form = doc.try_insert_object(object).unwrap();
            let parent = if inherited {
                let mut dictionary = parent_dictionary;
                let mut resources = local_resources;
                let mut xobjects = crate::objects::pdf_dict::PdfDict::default();
                xobjects.insert(
                    "Leaf".into(),
                    PdfObject::PdfReference(PdfReference::from_id(form)),
                );
                resources.insert("XObject".into(), PdfObject::PdfDict(xobjects));
                dictionary.insert("Resources".into(), PdfObject::PdfDict(resources));
                let object = PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        dictionary,
                        format!(
                            "BT /{} 12 Tf 1 Tc 14 TL ET /Leaf Do",
                            font.font.resource_name
                        )
                        .into_bytes(),
                    )
                    .unwrap(),
                );
                let original = object.clone();
                Some((doc.try_insert_object(object).unwrap(), original))
            } else {
                None
            };
            let invoked = parent.as_ref().map_or(form, |(handle, _)| *handle);
            let names = [
                doc.resources()
                    .register_existing(0, ResourceKind::Form, invoked)
                    .unwrap(),
                doc.resources()
                    .register_existing(1, ResourceKind::Form, invoked)
                    .unwrap(),
            ];
            for page in 0..2 {
                doc.replace_page_contents(
                    page,
                    format!("/{} Do /{} Do", names[page as usize], names[page as usize])
                        .into_bytes(),
                )
                .unwrap();
            }
            let page_fonts = doc
                .ctx()
                .resolve_owned(
                    doc.get_page(0)
                        .unwrap()
                        .resource()
                        .unwrap()
                        .get("Font")
                        .unwrap(),
                )
                .unwrap();
            let saved = doc.save_to_bytes().unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let nested = before.characters[0].nested_source.as_ref().unwrap();
            assert_eq!(nested.invocations.len(), if inherited { 2 } else { 1 });
            let position = TextCodePosition {
                operation: nested.operation_index,
                array_item: if event == "TJ" { Some(0) } else { None },
                offset: 0,
            };
            let end = if cross_event {
                TextCodePosition {
                    operation: before.characters[2].nested_source.as_ref().unwrap().operation_index,
                    array_item: None,
                    offset: if mixed { 1 } else { 2 },
                }
            } else { TextCodePosition {
                array_item: if event == "TJ" { Some(2) } else { None },
                offset: if event == "TJ" { 0 } else { 2 },
                ..position
            }};
            let options = || SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false,
                preserve_advance: preserve,
            };
            let baseline = pool.live_bytes();
            let tiny = OperationBudget::new(1);
            assert!(matches!(
                doc.replace_nested_text_event_range_with_subset_and_budget(
                    0,
                    &root,
                    nested,
                    position..end,
                    "BB",
                    options(),
                    &tiny
                ),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(tiny.live_bytes(), 0);
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let quota_fresh = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let nested = quota_fresh.characters[0].nested_source.as_ref().unwrap();
            let failed: PdfResult<()> = doc.edit_transaction(|document| {
                document.replace_nested_text_event_range_with_subset_and_budget(
                    0,
                    &root,
                    nested,
                    position..end,
                    "BB",
                    options(),
                    &pool,
                )?;
                Err(PdfError::ObjectError(
                    "after nested subset publication".into(),
                ))
            });
            assert!(
                matches!(failed, Err(PdfError::ObjectError(message)) if message == "after nested subset publication")
            );
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let fresh = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let nested = fresh.characters[0].nested_source.as_ref().unwrap();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: PdfResult<()> = doc.edit_transaction(|document| {
                    document.replace_nested_text_event_range_with_subset_and_budget(
                        0,
                        &root,
                        nested,
                        position..end,
                        "BB",
                        options(),
                        &pool,
                    )?;
                    panic!("after nested subset publication");
                });
            }));
            let payload = caught.expect_err("nested edit must reach deliberate publication panic");
            assert_eq!(
                payload.downcast_ref::<&str>().copied(),
                Some("after nested subset publication")
            );
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let fresh = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let nested = fresh.characters[0].nested_source.as_ref().unwrap();
            doc.replace_nested_text_event_range_with_subset_and_budget(
                0,
                &root,
                nested,
                position..end,
                "BB",
                options(),
                &pool,
            )
            .unwrap();
            {
                let mut current_page = doc.get_page(0).unwrap();
                let root = current_page.content_program().unwrap();
                let report = current_page
                    .extract_characters_paint_order_report()
                    .unwrap();
                let chain = current_page
                    .prepare_form_chain(&root, report.characters[0].nested_source.as_ref().unwrap())
                    .unwrap();
                let leaf = chain.last().unwrap();
                let fonts = doc
                    .ctx()
                    .resolve_owned(leaf.resource_scope.get("Font").unwrap())
                    .unwrap()
                    .into_dict()
                    .unwrap();
                let selected_name = if collision {
                    assert_eq!(fonts.get("FontEdit1"), Some(&reserved_font));
                    "FontEdit2"
                } else {
                    assert!(fonts.get("FontEdit2").is_none());
                    "FontEdit1"
                };
                assert_ne!(fonts.get(selected_name), Some(&reserved_font));
                assert!(fonts.get(selected_name).is_some());
                let bytes = leaf.program.source_bytes();
                let selected_token = format!("/{selected_name}");
                assert!(
                    bytes
                        .windows(selected_token.len())
                        .any(|bytes| bytes == selected_token.as_bytes()),
                    "new subset must use the collision-free leaf binding"
                );
                if event == "TJ" {
                    assert!(
                        bytes
                            .windows(b"-125 % retain".len())
                            .any(|bytes| bytes == b"-125 % retain")
                    );
                    assert!(bytes.windows(b"+75".len()).any(|bytes| bytes == b"+75"));
                }
            }
            assert_eq!(
                doc.ctx()
                    .resolve_owned(
                        doc.get_page(0)
                            .unwrap()
                            .resource()
                            .unwrap()
                            .get("Font")
                            .unwrap()
                    )
                    .unwrap(),
                page_fonts,
                "Form subset must not add a page Font binding"
            );
            assert_eq!(
                doc.ctx()
                    .resolve_owned(&PdfObject::PdfReference(PdfReference::from_id(form)))
                    .unwrap(),
                original
            );
            if let Some((handle, original)) = parent {
                assert_eq!(
                    doc.ctx()
                        .resolve_owned(&PdfObject::PdfReference(PdfReference::from_id(handle)))
                        .unwrap(),
                    original,
                    "original parent Form must remain unchanged"
                );
            }
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            let after = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                after
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                if cross_event { "BBAAAAA" } else { "BBAAAAAAA" }
            );
            assert_eq!(
                reopened
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                "AAAAAAAA"
            );
            let retained: &[(usize, usize)] = if cross_event {
                &[(3, 2)]
            } else {
                &[(1, 2), (2, 3), (3, 4)]
            };
            if mixed {
                assert_eq!(after[2].font_name.as_deref(), Some("Courier"));
            }
            for &(old, new) in retained {
                let delta = (before.characters[old].position.x() - after[new].position.x()).abs();
                let vertical_delta =
                    (before.characters[old].position.y() - after[new].position.y()).abs();
                assert!(
                    vertical_delta < 0.002,
                    "nested {event} suffix vertical delta {vertical_delta}"
                );
                if preserve {
                    assert!(delta < 0.002, "nested suffix delta {delta}");
                } else {
                    assert!(delta > 1.0);
                }
            }
            // The second invocation uses the unchanged original Form: neither
            // natural advancement nor compensation may move any of its glyphs.
            for offset in 0..4 {
                let old = 4 + offset;
                let new = if cross_event { 3 + offset } else { 5 + offset };
                let original = &before.characters[old];
                let unchanged = &after[new];
                assert_eq!(original.unicode, unchanged.unicode);
                assert!(
                    (original.position.x() - unchanged.position.x()).abs() < 0.002,
                    "nested {event} untouched invocation x changed"
                );
                assert!(
                    (original.position.y() - unchanged.position.y()).abs() < 0.002,
                    "nested {event} untouched invocation y changed"
                );
            }
            drop(font);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_cross_event_uses_original_indices_and_each_state() {
        use crate::{
            document::{
                PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                subset_text_edit::SubsetTextEditOptions,
            },
            page::content_program::TextCodePosition,
        };
        for (preserve, mixed) in [(false, false), (true, false), (false, true), (true, true)] {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(),
                cap_height: 60.0,
                stem_v: 80.0,
                serif: false,
            };
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let font = doc
                .resources()
                .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &pool)
                .unwrap();
            font.font.register_on_page(&doc, 1).unwrap();
            let tail_font = if mixed {
                let name = doc
                    .resources()
                    .register_standard_font(
                        0,
                        crate::document::resource_manager::StandardFont::Courier,
                    )
                    .unwrap();
                let shared = doc
                    .resources()
                    .register_standard_font(
                        1,
                        crate::document::resource_manager::StandardFont::Courier,
                    )
                    .unwrap();
                assert_eq!(name, shared);
                name
            } else {
                font.font.resource_name.clone()
            };
            let (one, two) = if mixed {
                ("(A)", "(AA)")
            } else {
                ("<0001>", "<00010001>")
            };
            let content = format!("BT /{} 12 Tf 2 Tc 20 50 Td <00010001> Tj /{} 18 Tf 3 Tc [{one} -125 % retain\n {two}] TJ {one} Tj ET", font.font.resource_name, tail_font).into_bytes();
            for page in 0..2 {
                doc.replace_page_contents(page, content.clone()).unwrap();
            }
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let start = snapshot
                .operations()
                .iter()
                .position(|op| op.operator.name() == "Tj")
                .unwrap();
            let end = snapshot
                .operations()
                .iter()
                .position(|op| op.operator.name() == "TJ")
                .unwrap();
            let range = TextCodePosition {
                operation: start,
                array_item: None,
                offset: 2,
            }..TextCodePosition {
                operation: end,
                array_item: Some(2),
                offset: if mixed { 1 } else { 2 },
            };
            let options = || SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false,
                preserve_advance: preserve,
            };
            let saved = doc.save_to_bytes().unwrap();
            let baseline = pool.live_bytes();
            let tiny = OperationBudget::new(1);
            let rejected = doc.replace_page_text_event_range_with_subset_and_budget(
                0,
                &snapshot,
                range.clone(),
                "B",
                options(),
                &tiny,
            );
            assert!(
                matches!(rejected, Err(PdfError::ResourceLimit(_))),
                "tiny budget must reach quota validation"
            );
            assert_eq!(tiny.live_bytes(), 0);
            assert_eq!(pool.live_bytes(), baseline);
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            let stale = doc.replace_page_text_event_range_with_subset_and_budget(
                0,
                &snapshot,
                range.clone(),
                "B",
                options(),
                &pool,
            );
            assert!(
                matches!(stale, Err(PdfError::ObjectError(message)) if message == "content snapshot is foreign, offline or stale")
            );
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let result: PdfResult<()> = doc.edit_transaction(|document| {
                document.replace_page_text_event_range_with_subset_and_budget(
                    0,
                    &snapshot,
                    range.clone(),
                    "B",
                    options(),
                    &pool,
                )?;
                Err(PdfError::ObjectError(
                    "after cross-event publication".into(),
                ))
            });
            assert!(
                matches!(result, Err(PdfError::ObjectError(message)) if message == "after cross-event publication")
            );
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: PdfResult<()> = doc.edit_transaction(|document| {
                    document.replace_page_text_event_range_with_subset_and_budget(
                        0,
                        &snapshot,
                        range.clone(),
                        "B",
                        options(),
                        &pool,
                    )?;
                    panic!("after cross-event publication");
                });
            }));
            let payload = caught.expect_err("published edit must reach deliberate panic");
            assert_eq!(
                payload.downcast_ref::<&str>().copied(),
                Some("after cross-event publication")
            );
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(pool.live_bytes(), baseline);
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            doc.replace_page_text_event_range_with_subset_and_budget(
                0,
                &snapshot,
                range,
                "B",
                options(),
                &pool,
            )
            .unwrap();
            assert_eq!(
                doc.get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                content
            );
            let current = doc.get_page(0).unwrap().content_program().unwrap();
            assert!(
                current
                    .source_bytes()
                    .windows(b"-125 % retain".len())
                    .any(|bytes| bytes == b"-125 % retain")
            );
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            let after = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                after
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                "ABAA"
            );
            if mixed {
                assert_eq!(after[2].font_name.as_deref(), Some("Courier"));
                assert_eq!(after[3].font_name.as_deref(), Some("Courier"));
            }
            assert_eq!(
                reopened
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                "AAAAAA"
            );
            for (old, new) in [(4, 2), (5, 3)] {
                let delta = (before[old].position.x() - after[new].position.x()).abs();
                if preserve {
                    assert!(delta < 0.002, "suffix delta {delta}");
                } else {
                    assert!(delta > 1.0);
                }
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
            drop(font);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_tj_cross_item_preserves_numbers_and_suffix_positions() {
        use crate::document::{
            PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
            subset_text_edit::SubsetTextEditOptions,
        };
        for preserve in [false, true] {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(),
                cap_height: 60.0,
                stem_v: 80.0,
                serif: false,
            };
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let original = doc
                .resources()
                .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &pool)
                .unwrap();
            doc.replace_page_contents(0, format!(
                "BT /{} 12 Tf 2 Tc -75 Tz 20 50 Td [<00010001> -125 % before\n <00010001> +75 % after\n <00010001>] TJ <0001> Tj ET",
                original.font.resource_name).into_bytes()).unwrap();
            original.font.register_on_page(&doc, 1).unwrap();
            let shared_content = doc
                .get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes()
                .to_vec();
            doc.replace_page_contents(1, shared_content.clone())
                .unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let mut snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let operation = snapshot
                .operations()
                .iter()
                .position(|op| op.operator.name() == "TJ")
                .unwrap();
            let options = || SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false,
                preserve_advance: preserve,
            };
            let saved_before = doc.save_to_bytes().unwrap();
            let baseline = pool.live_bytes();
            for (range, expected_error) in [
                (
                    (0, 1)..(4, 2),
                    "invalid or truncated CMap character at byte offset 0",
                ),
                (
                    (0, 2)..(4, 3),
                    "invalid or truncated CMap character at byte offset 0",
                ),
                ((1, 0)..(4, 2), "Subset TJ endpoint requires a string"),
                ((4, 2)..(0, 2), "Subset TJ range reversed"),
            ] {
                snapshot = doc.get_page(0).unwrap().content_program().unwrap();
                assert!(
                    matches!(doc.replace_page_text_array_range_with_subset_and_budget(
                        0,
                        &snapshot,
                        operation,
                        range,
                        "B",
                        options(),
                        &pool
                    ), Err(PdfError::FontError(message) | PdfError::ObjectError(message)) if message == expected_error),
                    "range rejection must reach the intended validation: {expected_error}"
                );
                assert_eq!(doc.save_to_bytes().unwrap(), saved_before);
                assert_eq!(pool.live_bytes(), baseline);
            }
            snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let tiny = OperationBudget::new(1);
            let tiny_result = doc.replace_page_text_array_range_with_subset_and_budget(
                0,
                &snapshot,
                operation,
                (0, 2)..(4, 2),
                "B",
                options(),
                &tiny,
            );
            assert!(
                matches!(&tiny_result, Err(PdfError::ResourceLimit(_))),
                "unexpected tiny-budget result: {tiny_result:?}"
            );
            assert_eq!(tiny.live_bytes(), 0);
            assert_eq!(doc.save_to_bytes().unwrap(), saved_before);
            snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let failed: PdfResult<()> = doc.edit_transaction(|document| {
                document.replace_page_text_array_range_with_subset_and_budget(
                    0,
                    &snapshot,
                    operation,
                    (0, 2)..(4, 2),
                    "B",
                    options(),
                    &pool,
                )?;
                Err(PdfError::ObjectError("after cross-item publication".into()))
            });
            assert!(
                matches!(failed, Err(PdfError::ObjectError(message)) if message == "after cross-item publication")
            );
            assert_eq!(doc.save_to_bytes().unwrap(), saved_before);
            assert_eq!(pool.live_bytes(), baseline);
            snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: PdfResult<()> = doc.edit_transaction(|document| {
                    document.replace_page_text_array_range_with_subset_and_budget(
                        0,
                        &snapshot,
                        operation,
                        (0, 2)..(4, 2),
                        "B",
                        options(),
                        &pool,
                    )?;
                    panic!("after cross-item publication");
                });
            }));
            assert!(caught.is_err());
            assert_eq!(doc.save_to_bytes().unwrap(), saved_before);
            assert_eq!(pool.live_bytes(), baseline);
            snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            doc.replace_page_text_array_range_with_subset_and_budget(
                0,
                &snapshot,
                operation,
                (0, 2)..(4, 2),
                "B",
                options(),
                &pool,
            )
            .unwrap();
            assert_eq!(
                doc.get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                shared_content
            );
            let current = doc.get_page(0).unwrap().content_program().unwrap();
            for token in [b"-125 % before".as_slice(), b"+75 % after".as_slice()] {
                assert!(
                    current
                        .source_bytes()
                        .windows(token.len())
                        .any(|bytes| bytes == token)
                );
            }
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            let after = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let shared = reopened
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                shared
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                "AAAAAAA"
            );
            assert_eq!(
                after
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                "ABAA"
            );
            assert_eq!(after[0].font_name.as_deref(), Some("AAAAAA+Demo"));
            assert_eq!(after[1].font_name.as_deref(), Some("BBBBBB+Demo"));
            assert_eq!(after[2].font_name.as_deref(), Some("AAAAAA+Demo"));
            for (old, new) in [(5, 2), (6, 3)] {
                let delta = (before[old].position.x() - after[new].position.x()).abs();
                if preserve {
                    assert!(delta < 0.002, "{old}: {delta}");
                } else {
                    assert!(delta > 1.0);
                }
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
            drop(original);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_tj_item_boundaries_and_public_rollback() {
        use crate::document::{
            PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
            subset_text_edit::SubsetTextEditOptions,
        };
        for (range, replacement, expected) in [
            (0..0, "B", "ABAAA"),
            (2..2, "B", "AABAA"),
            (4..4, "B", "AAABA"),
            (0..4, "", "AA"),
            (0..2, "B", "ABAA"),
            (2..4, "B", "AABA"),
        ] {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(),
                cap_height: 60.0,
                stem_v: 80.0,
                serif: false,
            };
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let original = doc
                .resources()
                .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &pool)
                .unwrap();
            original.font.register_on_page(&doc, 1).unwrap();
            let content = format!(
                "BT /{} 12 Tf [<0001> -125 <00010001> +75 <0001>] TJ ET",
                original.font.resource_name
            )
            .into_bytes();
            for page in 0..2 {
                doc.replace_page_contents(page, content.clone()).unwrap();
            }
            let snapshot = || doc.get_page(0).unwrap().content_program().unwrap();
            let options = || SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false,
                preserve_advance: true,
            };
            let original_saved = doc.save_to_bytes().unwrap();
            let current = snapshot();
            let operation = current
                .operations()
                .iter()
                .position(|op| op.operator.name() == "TJ")
                .unwrap();
            for (item, invalid) in [(1, 0..0), (2, 1..2), (2, 0..5)] {
                assert!(
                    doc.replace_page_text_array_item_with_subset_and_budget(
                        0,
                        &current,
                        operation,
                        item,
                        invalid,
                        replacement,
                        options(),
                        &pool
                    )
                    .is_err()
                );
                assert_eq!(doc.save_to_bytes().unwrap(), original_saved);
            }
            let failure: PdfResult<()> = doc.edit_transaction(|document| {
                document.replace_page_text_array_item_with_subset_and_budget(
                    0,
                    &current,
                    operation,
                    2,
                    range.clone(),
                    replacement,
                    options(),
                    &pool,
                )?;
                Err(PdfError::ObjectError("after TJ publication".into()))
            });
            assert!(failure.is_err());
            assert_eq!(doc.save_to_bytes().unwrap(), original_saved);
            let fresh = snapshot();
            doc.replace_page_text_array_item_with_subset_and_budget(
                0,
                &fresh,
                operation,
                2,
                range,
                replacement,
                options(),
                &pool,
            )
            .unwrap();
            let saved = doc.save_to_bytes().unwrap();
            let reopened =
                PdfDocument::from_bytes_with_mode(saved, crate::parser::syntax::ParseMode::Strict)
                    .unwrap();
            for (page, text) in [(0, expected), (1, "AAAA")] {
                let chars = reopened
                    .get_page(page)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                assert_eq!(
                    chars
                        .iter()
                        .filter_map(|c| c.unicode.as_deref())
                        .collect::<String>(),
                    text
                );
            }
            drop(original);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_compensation_preserves_suffix_and_following_positions() {
        use crate::document::{
            PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
            subset_text_edit::SubsetTextEditOptions,
        };
        for (preserve, event) in [
            (false, "Tj"),
            (true, "Tj"),
            (false, "'"),
            (true, "'"),
            (false, "\""),
            (true, "\""),
            (false, "TJ"),
            (true, "TJ"),
        ] {
            let source = fixture();
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let embedding = || TrueTypeEmbeddingOptions {
                postscript_name: "Demo".into(),
                cap_height: 60.0,
                stem_v: 80.0,
                serif: false,
            };
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let original = doc
                .resources()
                .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &pool)
                .unwrap();
            let show = match event {
                "'" => "<000100010001> '",
                "\"" => "9 4 <000100010001> \"",
                "TJ" => "[<0001> -125 % keep-before\n <0001> +75 % keep-after\n <0001>] TJ",
                _ => "<000100010001> Tj",
            };
            doc.replace_page_contents(
                0,
                format!(
                    "BT /{} 12 Tf 2 Tc 7 Tw -75 Tz 10 TL 20 50 Td {show} <0001> Tj ET",
                    original.font.resource_name
                )
                .into_bytes(),
            )
            .unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let operation = snapshot
                .operations()
                .iter()
                .position(|op| op.operator.name() == event)
                .unwrap();
            let options = SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false,
                preserve_advance: preserve,
            };
            if event == "TJ" {
                doc.replace_page_text_array_item_with_subset_and_budget(
                    0,
                    &snapshot,
                    operation,
                    2,
                    0..2,
                    "BB",
                    options,
                    &pool,
                )
                .unwrap();
                let current = doc.get_page(0).unwrap().content_program().unwrap();
                let source = current.source_bytes();
                assert!(
                    source
                        .windows(b"-125 % keep-before".len())
                        .any(|window| window == b"-125 % keep-before")
                );
                assert!(
                    source
                        .windows(b"+75 % keep-after".len())
                        .any(|window| window == b"+75 % keep-after")
                );
            } else {
                doc.replace_page_text_string_with_subset_and_budget(
                    0,
                    &snapshot,
                    operation,
                    2..4,
                    "BB",
                    options,
                    &pool,
                )
                .unwrap();
            }
            let saved = doc.save_to_bytes().unwrap();
            let reopened =
                PdfDocument::from_bytes_with_mode(saved, crate::parser::syntax::ParseMode::Strict)
                    .unwrap();
            let after = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                after
                    .iter()
                    .filter_map(|character| character.unicode.as_deref())
                    .collect::<String>(),
                "ABBAA"
            );
            assert_eq!(after[0].font_name.as_deref(), Some("AAAAAA+Demo"));
            assert_eq!(after[3].font_name.as_deref(), Some("AAAAAA+Demo"));
            for (old_index, new_index) in [(2, 3), (3, 4)] {
                let delta = (before[old_index].position.x() - after[new_index].position.x()).abs();
                if preserve {
                    assert!(delta < 0.002, "{old_index}: {delta}");
                } else {
                    assert!(delta > 1.0);
                }
                assert!(
                    (before[old_index].position.y() - after[new_index].position.y()).abs() < 0.002
                );
            }
            drop(original);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn automatic_subset_string_edit_is_atomic_and_isolates_shared_pages() {
        use crate::{
            document::{
                PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
                subset_text_edit::SubsetTextEditOptions,
            },
            parser::syntax::ParseMode,
        };
        let source = fixture();
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let embedding = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(),
            cap_height: 60.0,
            stem_v: 80.0,
            serif: false,
        };
        let budget = OperationBudget::new(65536);
        let original = doc
            .resources()
            .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &budget)
            .unwrap();
        let second = original.font.register_on_page(&doc, 1).unwrap();
        for (page, name) in [
            (0, original.font.resource_name.as_str()),
            (1, second.as_str()),
        ] {
            doc.replace_page_contents(
                page,
                format!("BT /{name} 12 Tf 20 50 Td <00010001> Tj <0001> Tj ET").into_bytes(),
            )
            .unwrap();
        }
        let before = doc.save_to_bytes().unwrap();
        let untouched_resources = doc.get_page(1).unwrap().resource().unwrap();
        let untouched_program = doc.get_page(1).unwrap().content_program().unwrap();
        let old_id = original.font.object_handle().object_id();
        let old_object = doc.resolve_object(old_id).unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let operation = snapshot
            .operations()
            .iter()
            .position(|op| op.operator.name() == "Tj")
            .unwrap();
        let options = || SubsetTextEditOptions {
            source: &source,
            embedding: embedding(),
            subset_tag: *b"BBBBBB",
            omit_layout_and_auxiliary: false,
            preserve_advance: false,
        };
        let short = OperationBudget::new(1);
        assert!(matches!(
            doc.replace_page_text_string_with_subset_and_budget(
                0,
                &snapshot,
                operation,
                0..2,
                "B",
                options(),
                &short
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(short.live_bytes(), 0);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        assert!(
            doc.replace_page_text_string_with_subset(0, &snapshot, operation, 1..2, "B", options())
                .is_err()
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        let failure: PdfResult<()> = doc.edit_transaction(|document| {
            document.replace_page_text_string_with_subset(
                0,
                &snapshot,
                operation,
                0..2,
                "B",
                options(),
            )?;
            Err(PdfError::ObjectError("outer rollback".into()))
        });
        assert!(failure.is_err());
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        // Rollback changes the revision: use a fresh snapshot, not bypass it.
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: PdfResult<()> = doc.edit_transaction(|document| {
                document.replace_page_text_string_with_subset(
                    0,
                    &snapshot,
                    operation,
                    0..2,
                    "B",
                    options(),
                )?;
                panic!("subset edit outer panic");
            });
        }));
        assert_eq!(
            caught.unwrap_err().downcast_ref::<&str>().copied(),
            Some("subset edit outer panic")
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let mut wrong_source = source.clone();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        let offset = directory.get_table("hmtx").unwrap().offset() as usize;
        wrong_source[offset + 1] ^= 1;
        let wrong_options = SubsetTextEditOptions {
            source: &wrong_source,
            embedding: embedding(),
            subset_tag: *b"CCCCCC",
            omit_layout_and_auxiliary: false,
            preserve_advance: false,
        };
        assert!(
            doc.replace_page_text_string_with_subset(
                0,
                &snapshot,
                operation,
                0..2,
                "B",
                wrong_options
            )
            .is_err()
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let edit_pool = OperationBudget::new(4 * 1024 * 1024);
        let neighbor = edit_pool.reserve(7).unwrap();
        doc.replace_page_text_string_with_subset_and_budget(
            0,
            &snapshot,
            operation,
            0..2,
            "B",
            options(),
            &edit_pool,
        )
        .unwrap();
        assert_eq!(edit_pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(edit_pool.live_bytes(), 0);
        let after = doc.save_to_bytes().unwrap();
        assert!(
            doc.replace_page_text_string_with_subset(0, &snapshot, operation, 0..2, "B", options())
                .is_err()
        );
        assert_eq!(doc.save_to_bytes().unwrap(), after);
        assert_eq!(doc.resolve_object(old_id).unwrap(), old_object);
        assert_eq!(
            doc.get_page(1).unwrap().resource().unwrap(),
            untouched_resources
        );
        assert_eq!(
            doc.get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            untouched_program.source_bytes()
        );
        let reopened = PdfDocument::from_bytes_with_mode(after, ParseMode::Strict).unwrap();
        for (page_index, expected) in [(0, "BAA"), (1, "AAA")] {
            let characters = reopened
                .get_page(page_index)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                characters
                    .iter()
                    .filter_map(|character| character.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
            assert_eq!(
                characters.last().unwrap().font_name.as_deref(),
                Some("AAAAAA+Demo")
            );
        }
        assert!(original.font.encoding().encode_text("B").is_err());
        drop(original);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn automatic_subset_empty_replacement_keeps_original_prefix_suffix() {
        use crate::document::{
            PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
            subset_text_edit::SubsetTextEditOptions,
        };
        let source = fixture();
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let embedding = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(),
            cap_height: 60.0,
            stem_v: 80.0,
            serif: false,
        };
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let original = doc
            .resources()
            .embed_truetype_subset_font(0, &source, "A", embedding(), *b"AAAAAA", false, &pool)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!(
                "BT /{} 12 Tf <000100010001> Tj <0001> Tj ET",
                original.font.resource_name
            )
            .into_bytes(),
        )
        .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let operation = snapshot
            .operations()
            .iter()
            .position(|op| op.operator.name() == "Tj")
            .unwrap();
        doc.replace_page_text_string_with_subset_and_budget(
            0,
            &snapshot,
            operation,
            2..4,
            "",
            SubsetTextEditOptions {
                source: &source,
                embedding: embedding(),
                subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false,
                preserve_advance: true,
            },
            &pool,
        )
        .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        let characters = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            characters
                .iter()
                .filter_map(|c| c.unicode.as_deref())
                .collect::<String>(),
            "AAA"
        );
        assert!(
            characters
                .iter()
                .all(|c| c.font_name.as_deref() == Some("AAAAAA+Demo"))
        );
        drop(original);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn subset_source_bounds_aggregate_comparison_work_and_retries() {
        let base = fixture();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&base)).unwrap();
        let hint_program = vec![0u8; 4096];
        let mut tables: Vec<_> = directory
            .table_records()
            .map(|record| {
                let tag: [u8; 4] = record.tag().as_bytes().try_into().unwrap();
                let start = record.offset() as usize;
                (tag, &base[start..start + record.length() as usize])
            })
            .collect();
        tables.push((*b"fpgm", &hint_program));
        let limits = Limits::default();
        let pool = OperationBudget::new(131072);
        let source = write_sfnt(&tables, &limits, &pool).unwrap();
        let program = TrueTypeProgram::try_new(source.as_slice()).unwrap();
        let encoding = UnicodeCidEncoding::from_text(&program, "A", limits).unwrap();
        let subset = build_subset(
            source.as_slice(),
            &encoding,
            "ABCDEF+Demo",
            false,
            &limits,
            &pool,
        )
        .unwrap();
        let restricted = Limits {
            max_decoded_bytes: source.as_slice().len().max(subset.bytes.as_slice().len()),
            ..limits
        };
        assert!(
            matches!(validate_subset_source(subset.bytes.as_slice(), source.as_slice(), restricted), Err(PdfError::ResourceLimit(message)) if message == "subset source comparison bytes")
        );
        validate_subset_source(subset.bytes.as_slice(), source.as_slice(), limits).unwrap();
        drop(subset);
        drop(source);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn subset_source_checks_preserved_metrics_and_retained_glyphs() {
        let source = fixture();
        let limits = Limits::default();
        let program = TrueTypeProgram::try_new(&source).unwrap();
        let encoding = UnicodeCidEncoding::from_text(&program, "A", limits).unwrap();
        let pool = OperationBudget::new(65536);
        let subset =
            build_subset(&source, &encoding, "ABCDEF+Demo", false, &limits, &pool).unwrap();
        validate_subset_source(subset.bytes.as_slice(), &source, limits).unwrap();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        for (tag, relative) in [("hmtx", 1usize), ("glyf", 18), ("head", 19), ("head", 16)] {
            let mut wrong = source.clone();
            let offset = directory.get_table(tag).unwrap().offset() as usize + relative;
            wrong[offset] ^= 1;
            assert!(
                validate_subset_source(subset.bytes.as_slice(), &wrong, limits).is_err(),
                "{tag}"
            );
        }
        assert!(validate_subset_source(subset.bytes.as_slice(), &source[..10], limits).is_err());
        validate_subset_source(subset.bytes.as_slice(), &source, limits).unwrap();
        drop(subset);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn registered_subset_descriptor_uses_rebuilt_bounds_after_strict_reopen() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions},
            parser::syntax::ParseMode,
        };
        let mut source = fixture();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        let offset = directory.get_table("head").unwrap().offset() as usize;
        for (slot, value) in source[offset + 36..offset + 44]
            .chunks_exact_mut(2)
            .zip([-200i16, -300, 400, 500])
        {
            slot.copy_from_slice(&value.to_be_bytes());
        }
        let original = source.clone();
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let budget = OperationBudget::new(65536);
        let registered = doc
            .resources()
            .embed_truetype_subset_font(
                0,
                &source,
                "A",
                TrueTypeEmbeddingOptions {
                    postscript_name: "Demo".into(),
                    cap_height: 60.0,
                    stem_v: 80.0,
                    serif: false,
                },
                *b"ABCDEF",
                false,
                &budget,
            )
            .unwrap();
        let font_id = registered.font.object_handle().object_id();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let font = document.resolve_object(font_id).unwrap();
            let parsed_font =
                crate::font::pdf_font::Font::try_new(font.as_dict().unwrap(), document.ctx())
                    .unwrap();
            let source_program = TrueTypeProgram::try_new(&source).unwrap();
            let baseline = budget.live_bytes();
            let program = crate::page::content_program::ContentProgram::from_decoded_bytes(
                b"[<00010001> -125 <00010001> +75 <00010001>] TJ".to_vec(),
                Default::default(),
            )
            .unwrap();
            let array = program.operations()[0].operator.get_as_array(0).unwrap();
            for (range, text, expected) in [
                ((0, 2)..(4, 2), "B", "ABA"),
                ((0, 2)..(0, 2), "B", "ABAAAAA"),
                ((0, 0)..(4, 4), "B", "B"),
                ((0, 0)..(4, 4), "", ""),
            ] {
                let planned = plan_subset_text_array_range(
                    &parsed_font,
                    &source_program,
                    array,
                    range,
                    text,
                    Limits::default(),
                    &budget,
                )
                .unwrap();
                assert_eq!(planned.as_slice(), expected.as_bytes());
                drop(planned);
                assert_eq!(budget.live_bytes(), baseline);
            }
            for (range, text) in [
                ((0, 1)..(4, 2), "B"),
                ((0, 0)..(4, 3), "B"),
                ((1, 0)..(4, 2), "B"),
                ((4, 2)..(0, 2), "B"),
                ((0, 0)..(4, 5), "B"),
                ((0, 0)..(4, 4), "C"),
            ] {
                assert!(
                    plan_subset_text_array_range(
                        &parsed_font,
                        &source_program,
                        array,
                        range,
                        text,
                        Limits::default(),
                        &budget
                    )
                    .is_err()
                );
                assert_eq!(budget.live_bytes(), baseline);
            }
            assert!(
                matches!(plan_subset_text_array_range(&parsed_font, &source_program,
                array, (0, 0)..(4, 4), "B", Limits { max_text_chars: 5, ..Limits::default() },
                &budget), Err(PdfError::ResourceLimit(message)) if message == "subset TJ input characters")
            );
            assert_eq!(budget.live_bytes(), baseline);
            let tiny = OperationBudget::new(1);
            assert!(
                matches!(plan_subset_text_array_range(&parsed_font, &source_program,
                array, (0, 2)..(0, 2), "B", Limits { max_text_chars: 6, ..Limits::default() },
                &budget), Err(PdfError::ResourceLimit(message)) if message == "subset TJ planned characters")
            );
            assert!(
                matches!(plan_subset_text_array_range(&parsed_font, &source_program,
                array, (0, 0)..(4, 4), "", Limits { max_decoded_bytes: 11, ..Limits::default() },
                &budget), Err(PdfError::ResourceLimit(message)) if message == "subset TJ input bytes")
            );
            let invalid = crate::page::content_program::ContentProgram::from_decoded_bytes(
                b"[<0001> -125 <ffff> +75 <0001>] TJ".to_vec(),
                Default::default(),
            )
            .unwrap();
            assert!(
                plan_subset_text_array_range(
                    &parsed_font,
                    &source_program,
                    invalid.operations()[0].operator.get_as_array(0).unwrap(),
                    (0, 0)..(4, 2),
                    "",
                    Limits::default(),
                    &budget
                )
                .is_err()
            );
            assert_eq!(budget.live_bytes(), baseline);
            assert!(
                plan_subset_text_array_range(
                    &parsed_font,
                    &source_program,
                    array,
                    (0, 2)..(4, 2),
                    "B",
                    Limits::default(),
                    &tiny
                )
                .is_err()
            );
            assert_eq!(tiny.live_bytes(), 0);
            for (range, text, expected) in [
                (0..2, "B", "BA"),
                (2..2, "B", "ABA"),
                (2..4, "B", "AB"),
                (0..4, "", ""),
            ] {
                let planned = plan_subset_string(
                    &parsed_font,
                    &source_program,
                    &[0, 1, 0, 1],
                    range,
                    text,
                    Limits::default(),
                    &budget,
                )
                .unwrap();
                assert_eq!(planned.as_slice(), expected.as_bytes());
                drop(planned);
                assert_eq!(budget.live_bytes(), baseline);
            }
            for (range, text) in [
                (1..2, "B"),
                (0..3, "B"),
                (4..2, "B"),
                (0..5, "B"),
                (0..2, "C"),
            ] {
                assert!(
                    plan_subset_string(
                        &parsed_font,
                        &source_program,
                        &[0, 1, 0, 1],
                        range,
                        text,
                        Limits::default(),
                        &budget
                    )
                    .is_err()
                );
                assert_eq!(budget.live_bytes(), baseline);
            }
            assert!(
                matches!(plan_subset_string(&parsed_font, &source_program, &[0, 1, 0, 1], 2..2, "B", Limits { max_text_chars: 2, ..Limits::default() }, &budget), Err(PdfError::ResourceLimit(message)) if message == "subset planned characters")
            );
            assert_eq!(budget.live_bytes(), baseline);
            let decoded = decode_subset_codes(
                &parsed_font,
                &source_program,
                &[0, 1, 0, 1],
                Limits::default(),
                &budget,
            )
            .unwrap();
            assert_eq!(decoded.as_slice(), b"AA");
            drop(decoded);
            assert_eq!(budget.live_bytes(), baseline);
            let short = OperationBudget::new(1);
            assert!(
                decode_subset_codes(
                    &parsed_font,
                    &source_program,
                    &[0, 1, 0, 1],
                    Limits::default(),
                    &short
                )
                .is_err()
            );
            assert_eq!(short.live_bytes(), 0);
            let mut visited = Vec::new();
            visit_subset_scalars(
                &parsed_font,
                &source_program,
                &[0, 1, 0, 1],
                Limits::default(),
                |scalar, gid| {
                    visited.push((scalar, gid));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(visited, [('A', 1), ('A', 1)]);
            assert!(
                matches!(visit_subset_scalars(&parsed_font, &source_program, &[0, 1], Limits::default(), |_, _| Err(PdfError::ObjectError("visitor refusal".into()))), Err(PdfError::ObjectError(message)) if message == "visitor refusal")
            );
            assert_eq!(
                validate_subset_codes(
                    &parsed_font,
                    &source_program,
                    &[0, 1, 0, 1],
                    Limits::default()
                )
                .unwrap(),
                2
            );
            assert!(
                validate_subset_codes(&parsed_font, &source_program, &[0, 2], Limits::default())
                    .is_err()
            );
            assert!(
                validate_subset_codes(&parsed_font, &source_program, &[0], Limits::default())
                    .is_err()
            );
            let mut wrong_mapping = source.clone();
            let cmap_offset = directory.get_table("cmap").unwrap().offset() as usize;
            wrong_mapping[cmap_offset + 36..cmap_offset + 40].copy_from_slice(&2u32.to_be_bytes());
            let wrong_program = TrueTypeProgram::try_new(&wrong_mapping).unwrap();
            assert!(
                validate_subset_codes(&parsed_font, &wrong_program, &[0, 1], Limits::default())
                    .is_err()
            );
            let descendant_id = font
                .get_attr("DescendantFonts")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap()
                .as_reference()
                .unwrap()
                .id();
            let descendant = document.resolve_object(descendant_id).unwrap();
            let descriptor_id = descendant
                .get_attr("FontDescriptor")
                .unwrap()
                .as_reference()
                .unwrap()
                .id();
            let descriptor = document.resolve_object(descriptor_id).unwrap();
            let bounds = descriptor.get_attr("FontBBox").unwrap().as_array().unwrap();
            for (index, expected) in [0.0, 0.0, 30.0, 60.0].into_iter().enumerate() {
                assert_eq!(bounds.get(index).unwrap().as_f32(), Some(expected));
            }
            assert!(bounds.get(4).is_none());
        }
        assert_eq!(source, original);
        drop(registered);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn new_character_uses_new_local_subset_without_mutating_shared_font() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions},
            page::{content_builder::ContentBuilder, content_program::OperationReplacement},
            parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let options = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(),
            cap_height: 60.0,
            stem_v: 80.0,
            serif: false,
        };
        let source = fixture();
        let budget = OperationBudget::new(65536);
        let original = doc
            .resources()
            .embed_truetype_subset_font(0, &source, "A", options(), *b"AAAAAA", false, &budget)
            .unwrap();
        let second_resource = original.font.register_on_page(&doc, 1).unwrap();
        for (page, resource) in [
            (0, original.font.resource_name.as_str()),
            (1, second_resource.as_str()),
        ] {
            doc.replace_page_contents(
                page,
                format!("BT /{resource} 12 Tf 20 50 Td <0001> Tj <0001> Tj ET\n").into_bytes(),
            )
            .unwrap();
        }
        let old_id = original.font.object_handle().object_id();
        let old_object = doc.resolve_object(old_id).unwrap();
        let second_resources = doc.get_page(1).unwrap().resource().unwrap();
        let second_content = doc
            .get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes()
            .to_vec();
        let before_failure = doc.save_to_bytes().unwrap();
        let replacement = |resource: &str| -> PdfResult<ContentBuilder> {
            let mut builder = ContentBuilder::new();
            builder.save_state();
            builder.try_set_font(resource, 12.0)?;
            builder.show_encoded_text(&[0, 1])?;
            builder.restore_state();
            Ok(builder)
        };
        let failure: PdfResult<()> = doc.edit_transaction(|doc| {
            let new = doc.resources().embed_truetype_subset_font(
                0,
                &source,
                "B",
                options(),
                *b"BBBBBB",
                false,
                &budget,
            )?;
            let snapshot = doc.get_page(0)?.content_program()?;
            let builder = replacement(&new.font.resource_name)?;
            doc.replace_page_operation_sequences(
                0,
                &snapshot,
                &[OperationReplacement {
                    index: 3,
                    source: builder.as_bytes(),
                    operation_count: 4,
                }],
            )?;
            Err(PdfError::DocumentError("intentional outer rollback".into()))
        });
        assert!(failure.is_err());
        assert_eq!(doc.save_to_bytes().unwrap(), before_failure);
        doc.edit_transaction(|doc| {
            let new = doc.resources().embed_truetype_subset_font(
                0,
                &source,
                "B",
                options(),
                *b"BBBBBB",
                false,
                &budget,
            )?;
            assert_ne!(new.font.object_handle().object_id(), old_id);
            let snapshot = doc.get_page(0)?.content_program()?;
            let builder = replacement(&new.font.resource_name)?;
            doc.replace_page_operation_sequences(
                0,
                &snapshot,
                &[OperationReplacement {
                    index: 3,
                    source: builder.as_bytes(),
                    operation_count: 4,
                }],
            )
        })
        .unwrap();
        assert_eq!(doc.resolve_object(old_id).unwrap(), old_object);
        assert_eq!(
            doc.get_page(1).unwrap().resource().unwrap(),
            second_resources
        );
        assert_eq!(
            doc.get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            second_content
        );
        assert!(original.font.encoding().encode_text("B").is_err());
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for (index, expected) in [(0, "BA"), (1, "AA")] {
            let characters = reopened
                .get_page(index)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                characters
                    .iter()
                    .filter_map(|c| c.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
            assert_eq!(
                characters.last().unwrap().font_name.as_deref(),
                Some("AAAAAA+Demo")
            );
        }
        drop(original);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn public_subset_registration_budget_boundary_rolls_back_and_retries() {
        let source = fixture();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        let mut header = [0u8; 36];
        header[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        header[34..36].copy_from_slice(&3u16.to_be_bytes());
        let vertical_metrics = [0u8; 12];
        let mut tables: Vec<([u8; 4], &[u8])> = directory.table_records().map(|record| {
            let start = record.offset() as usize;
            (record.tag().as_bytes().try_into().unwrap(),
                &source[start..start + record.length() as usize])
        }).collect();
        tables.push((*b"vhea", &header));
        tables.push((*b"vmtx", &vertical_metrics));
        let vertical_source = write_sfnt(
            &tables, &Limits::default(), &OperationBudget::new(65536),
        ).unwrap();
        for vertical in [false, true] {
        let source = if vertical { vertical_source.as_slice() } else { &source };
        let embed = |doc: &crate::document::PdfDocument<std::io::Cursor<Vec<u8>>>, pool: &OperationBudget| {
            let resources = doc.resources();
            let register = if vertical {
                crate::document::resource_manager::ResourceManager::embed_truetype_vertical_subset_font
            } else {
                crate::document::resource_manager::ResourceManager::embed_truetype_subset_font
            };
            register(&resources,
                0, &source, "A",
                crate::document::embedded_font::TrueTypeEmbeddingOptions {
                    postscript_name: "Demo".into(), cap_height: 60.0,
                    stem_v: 80.0, serif: false,
                }, *b"BUDGET", false, pool,
            )
        };
        let doc = crate::document::PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let pool = OperationBudget::new(16 * 1024 * 1024);
        let registered = embed(&doc, &pool).unwrap();
        let peak = pool.peak_bytes();
        drop(registered);
        assert_eq!(pool.live_bytes(), 0);
        for maximum in [peak - 1, peak] {
            let doc = crate::document::PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let before = doc.save_to_bytes().unwrap();
            let tight = OperationBudget::new(maximum);
            let result = embed(&doc, &tight);
            assert_eq!(result.is_ok(), maximum == peak);
            if result.is_err() {
                assert_eq!(doc.save_to_bytes().unwrap(), before);
            }
            drop(result);
            assert_eq!(tight.live_bytes(), 0);
        }
        let doc = crate::document::PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let before = doc.save_to_bytes().unwrap();
        let tight = OperationBudget::new(peak);
        let neighbor = tight.reserve(1).unwrap();
        assert!(embed(&doc, &tight).is_err());
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        assert_eq!(tight.live_bytes(), 1);
        drop(neighbor);
        drop(embed(&doc, &tight).unwrap());
        assert_eq!(tight.live_bytes(), 0);
        let saved = doc.save_to_bytes().unwrap();
        if vertical {
            assert!(saved.windows(11).any(|bytes| bytes == b"/Identity-V"));
            assert!(saved.windows(3).any(|bytes| bytes == b"/W2"));
            assert!(saved.windows(3).any(|bytes| bytes == b"/W "));
        }
        crate::document::PdfDocument::from_bytes_with_mode(
            saved, crate::parser::syntax::ParseMode::Strict,
        ).unwrap();
        }
    }

    #[test]
    fn original_gid_vertical_metrics_preserve_and_reject_missing_or_short_pair() {
        let source = fixture();
        assert!(TrueTypeProgram::checked_vertical_metrics(&source, Limits::default()).unwrap().is_none());
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        let mut vertical = [0u8; 36];
        vertical[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        vertical[34..36].copy_from_slice(&3u16.to_be_bytes());
        let metrics = [0u8; 12];
        for (include_header, include_metrics, metrics_length) in [
            (true, true, 12usize),
            (true, false, 0),
            (false, true, 12),
            (true, true, 11),
        ] {
            let mut tables: Vec<([u8; 4], &[u8])> = directory
                .table_records()
                .map(|record| {
                    let start = record.offset() as usize;
                    (
                        record.tag().as_bytes().try_into().unwrap(),
                        &source[start..start + record.length() as usize],
                    )
                })
                .collect();
            if include_header {
                tables.push((*b"vhea", &vertical));
            }
            if include_metrics {
                tables.push((*b"vmtx", &metrics[..metrics_length]));
            }
            let input =
                write_sfnt(&tables, &Limits::default(), &OperationBudget::new(65536)).unwrap();
            let inspected = TrueTypeProgram::checked_vertical_metrics(input.as_slice(), Limits::default());
            if include_header && include_metrics && metrics_length == 12 {
                let inspected = inspected.unwrap().unwrap();
                for gid in 0..3 {
                    assert_eq!(inspected.metric(gid).unwrap(), (0, 0));
                }
            } else {
                assert!(inspected.is_err());
            }
            let doc = crate::document::PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let before = doc.save_to_bytes().unwrap();
            let registered = doc.resources().embed_truetype_vertical_font(
                0, input.as_slice().to_vec(), "A",
                crate::document::embedded_font::TrueTypeEmbeddingOptions {
                    postscript_name: "Demo".into(), cap_height: 60.0,
                    stem_v: 80.0, serif: false,
                },
            );
            if include_header && include_metrics && metrics_length == 12 {
                let registered = registered.unwrap();
                doc.replace_page_contents(0, format!(
                    "BT /{} 24 Tf 48 760 Td <0001> Tj ET", registered.resource_name,
                ).into_bytes()).unwrap();
                let saved = doc.save_to_bytes().unwrap();
                assert!(saved.windows(11).any(|bytes| bytes == b"/Identity-V"));
                assert!(saved.windows(3).any(|bytes| bytes == b"/W2"));
                let reopened = crate::document::PdfDocument::from_bytes_with_mode(
                    saved, crate::parser::syntax::ParseMode::Strict,
                ).unwrap();
                let chars = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap();
                assert_eq!(chars.len(), 1);
                assert_eq!(chars[0].unicode.as_deref(), Some("A"));
            } else {
                assert!(registered.is_err());
                assert_eq!(doc.save_to_bytes().unwrap(), before);
            }
            let subset_doc = crate::document::PdfDocument::new().unwrap();
            subset_doc.add_page(0).unwrap();
            let subset_before = subset_doc.save_to_bytes().unwrap();
            // Registration plus full public edit planning includes the font
            // resolver/CMap scratch; this is a semantic round-trip test, not
            // an exact operation-pool boundary test.
            let subset_budget = OperationBudget::new(16 * 1024 * 1024);
            let vertical_subset = subset_doc.resources().embed_truetype_vertical_subset_font(
                0, input.as_slice(), "A",
                crate::document::embedded_font::TrueTypeEmbeddingOptions {
                    postscript_name: "Demo".into(), cap_height: 60.0,
                    stem_v: 80.0, serif: false,
                }, *b"VERTAA", false, &subset_budget,
            );
            if include_header && include_metrics && metrics_length == 12 {
                let vertical_subset = vertical_subset.unwrap();
                subset_doc.add_page(1).unwrap();
                let shared_name = vertical_subset.font.register_on_page(&subset_doc, 1).unwrap();
                subset_doc.replace_page_contents(1, format!(
                    "BT /{shared_name} 24 Tf 48 760 Td <0001> Tj ET",
                ).into_bytes()).unwrap();
                let original_font_id = vertical_subset.font.object_handle().object_id();
                let original_font = subset_doc.resolve_object(original_font_id).unwrap();
                subset_doc.replace_page_contents(0, format!(
                    "BT /{} 24 Tf 48 760 Td <0001> Tj ET", vertical_subset.font.resource_name,
                ).into_bytes()).unwrap();
                let saved = subset_doc.save_to_bytes().unwrap();
                assert!(saved.windows(11).any(|bytes| bytes == b"/Identity-V"));
                assert!(saved.windows(3).any(|bytes| bytes == b"/W2"));
                let reopened = crate::document::PdfDocument::from_bytes_with_mode(
                    saved, crate::parser::syntax::ParseMode::Strict,
                ).unwrap();
                let chars = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap();
                assert_eq!(chars[0].unicode.as_deref(), Some("A"));
                let root = subset_doc.get_page(0).unwrap().content_program().unwrap();
                let edit_options = || crate::document::subset_text_edit::SubsetTextEditOptions {
                    source: input.as_slice(),
                    embedding: crate::document::embedded_font::TrueTypeEmbeddingOptions {
                        postscript_name: "Demo".into(), cap_height: 60.0,
                        stem_v: 80.0, serif: false,
                    }, subset_tag: *b"VERTAB", omit_layout_and_auxiliary: false,
                    preserve_advance: true,
                };
                let unchanged = subset_doc.save_to_bytes().unwrap();
                let baseline_live = subset_budget.live_bytes();
                let tiny = OperationBudget::new(1);
                assert!(subset_doc.replace_page_text_string_with_subset_and_budget(
                    0, &root, 3, 0..2, "AA", edit_options(), &tiny,
                ).is_err());
                assert_eq!(tiny.live_bytes(), 0);
                assert_eq!(subset_doc.save_to_bytes().unwrap(), unchanged);
                let root = subset_doc.get_page(0).unwrap().content_program().unwrap();
                let failure: crate::error::PdfResult<()> = subset_doc.edit_transaction(|doc| {
                    doc.replace_page_text_string_with_subset_and_budget(
                        0, &root, 3, 0..2, "AA", edit_options(), &subset_budget,
                    )?;
                    Err(PdfError::DocumentError("vertical outer rollback".into()))
                });
                assert!(matches!(failure, Err(PdfError::DocumentError(message)) if message == "vertical outer rollback"));
                assert_eq!(subset_doc.save_to_bytes().unwrap(), unchanged);
                assert_eq!(subset_budget.live_bytes(), baseline_live);
                let root = subset_doc.get_page(0).unwrap().content_program().unwrap();
                let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: crate::error::PdfResult<()> = subset_doc.edit_transaction(|doc| {
                        doc.replace_page_text_string_with_subset_and_budget(
                            0, &root, 3, 0..2, "AA", edit_options(), &subset_budget,
                        ).unwrap();
                        panic!("vertical outer panic rollback");
                    });
                }));
                let payload = panic_result.expect_err("outer panic must propagate");
                assert_eq!(payload.downcast_ref::<&str>(), Some(&"vertical outer panic rollback"));
                assert_eq!(subset_doc.save_to_bytes().unwrap(), unchanged);
                assert_eq!(subset_budget.live_bytes(), baseline_live);
                let root = subset_doc.get_page(0).unwrap().content_program().unwrap();
                subset_doc.replace_page_text_string_with_subset_and_budget(
                    0, &root, 3, 0..2, "AA",
                    crate::document::subset_text_edit::SubsetTextEditOptions {
                        source: input.as_slice(),
                        embedding: crate::document::embedded_font::TrueTypeEmbeddingOptions {
                            postscript_name: "Demo".into(), cap_height: 60.0,
                            stem_v: 80.0, serif: false,
                        }, subset_tag: *b"VERTAB", omit_layout_and_auxiliary: false,
                        preserve_advance: true,
                    }, &subset_budget,
                ).unwrap();
                let reopened = crate::document::PdfDocument::from_bytes_with_mode(
                    subset_doc.save_to_bytes().unwrap(), crate::parser::syntax::ParseMode::Strict,
                ).unwrap();
                let chars = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap();
                assert_eq!(chars.iter().filter_map(|c| c.unicode.as_deref()).collect::<String>(), "AA");
                let shared_chars = reopened.get_page(1).unwrap().extract_characters_paint_order().unwrap();
                assert_eq!(shared_chars.iter().filter_map(|c| c.unicode.as_deref()).collect::<String>(), "A");
                assert_eq!(subset_doc.resolve_object(original_font_id).unwrap(), original_font);
                drop(vertical_subset);
            } else {
                assert!(vertical_subset.is_err());
                assert_eq!(subset_doc.save_to_bytes().unwrap(), subset_before);
            }
            assert_eq!(subset_budget.live_bytes(), 0);
            eprintln!("Vertical public edit tracked peak: {}", subset_budget.peak_bytes());
            assert!(subset_budget.peak_bytes() < 8 * 1024 * 1024);
            let program = TrueTypeProgram::try_new(input.as_slice()).unwrap();
            let encoding = UnicodeCidEncoding::from_text(&program, "A", Limits::default()).unwrap();
            let budget = OperationBudget::new(65536);
            let result = build_subset(
                input.as_slice(),
                &encoding,
                "ABCDEF+Demo",
                false,
                &Limits::default(),
                &budget,
            );
            if include_header && include_metrics && metrics_length == 12 {
                let subset = result.unwrap();
                let reopened =
                    Sfnt::try_new(&mut TrueTypeReader::new(subset.bytes.as_slice())).unwrap();
                for (tag, expected) in [("vhea", &vertical[..]), ("vmtx", &metrics[..])] {
                    let record = reopened.get_table(tag).unwrap();
                    let start = record.offset() as usize;
                    assert_eq!(
                        &subset.bytes.as_slice()[start..start + record.length() as usize],
                        expected
                    );
                }
                drop(subset);
            } else {
                assert!(result.is_err());
            }
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn public_subset_object_limit_failure_rolls_back_inserted_font_objects() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions},
            objects::{PdfObject, object_id::ObjectId},
            parser::options::ParseOptions,
        };
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_objects: 10,
                ..Limits::default()
            },
            ..ParseOptions::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        for expected in 5..=8 {
            assert_eq!(
                doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
                expected
            );
        }
        let before: Vec<_> = (1..=8)
            .map(|id| doc.resolve_object(ObjectId::new(id, 0)).unwrap())
            .collect();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let budget = OperationBudget::new(65536);
        assert!(matches!(
            doc.resources().embed_truetype_subset_font(
                0,
                &fixture(),
                "A",
                TrueTypeEmbeddingOptions {
                    postscript_name: "Demo".into(),
                    cap_height: 60.0,
                    stem_v: 80.0,
                    serif: false,
                },
                *b"ABCDEF",
                false,
                &budget,
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 0);
        for (index, object) in before.iter().enumerate() {
            assert_eq!(
                &doc.resolve_object(ObjectId::new(index as u32 + 1, 0))
                    .unwrap(),
                object
            );
        }
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        assert_eq!(
            doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
            9
        );
    }

    #[test]
    fn public_subset_registration_round_trips_shared_pages_and_refuses_without_mutation() {
        use crate::{
            document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions},
            parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let options = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(),
            cap_height: 60.0,
            stem_v: 80.0,
            serif: false,
        };
        let source = fixture();
        let before = doc.save_to_bytes().unwrap();
        let tiny = OperationBudget::new(1);
        assert!(
            doc.resources()
                .embed_truetype_subset_font(0, &source, "A", options(), *b"ABCDEF", false, &tiny,)
                .is_err()
        );
        assert_eq!(tiny.live_bytes(), 0);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        let budget = OperationBudget::new(65536);
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        let mut prohibited = source.clone();
        let rights_offset = directory.get_table("OS/2").unwrap().offset() as usize + 8;
        // OS/2 versions 0/1 ignore the later no-subsetting flag.
        prohibited[rights_offset - 8..rights_offset - 6].copy_from_slice(&3u16.to_be_bytes());
        prohibited[rights_offset..rights_offset + 2].copy_from_slice(&0x0100u16.to_be_bytes());
        assert!(
            doc.resources()
                .embed_truetype_subset_font(
                    0,
                    &prohibited,
                    "A",
                    options(),
                    *b"ABCDEF",
                    false,
                    &budget,
                )
                .is_err()
        );
        assert!(
            doc.resources()
                .embed_truetype_subset_font(0, &source, "A", options(), *b"abcDEF", false, &budget,)
                .is_err()
        );
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        for invalid in ["", "Bad+Name", "Bad/Name"] {
            let mut invalid_options = options();
            invalid_options.postscript_name = invalid.into();
            assert!(
                doc.resources()
                    .embed_truetype_subset_font(
                        0,
                        &source,
                        "A",
                        invalid_options,
                        *b"ABCDEF",
                        false,
                        &budget,
                    )
                    .is_err()
            );
            assert_eq!(budget.live_bytes(), 0);
            assert_eq!(doc.save_to_bytes().unwrap(), before);
        }
        let registered = doc
            .resources()
            .embed_truetype_subset_font(0, &source, "A", options(), *b"ABCDEF", false, &budget)
            .unwrap();
        assert!(registered.omitted_tables().is_empty());
        assert_eq!(
            registered.font.encoding().encode_text("AA").unwrap(),
            [0, 1, 0, 1]
        );
        assert!(registered.font.encoding().encode_text("B").is_err());
        let second_name = registered.font.register_on_page(&doc, 1).unwrap();
        for (page, resource) in [
            (0, registered.font.resource_name.as_str()),
            (1, second_name.as_str()),
        ] {
            doc.replace_page_contents(
                page,
                format!("BT /{resource} 12 Tf 20 50 Td <00010001> Tj ET\n").into_bytes(),
            )
            .unwrap();
        }
        let saved = doc.save_to_bytes().unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
        for index in 0..2 {
            let mut page = reopened.get_page(index).unwrap();
            let characters = page.extract_characters_paint_order().unwrap();
            assert_eq!(characters.len(), 2);
            for character in characters {
                assert_eq!(character.unicode.as_deref(), Some("A"));
                assert_eq!(character.font_name.as_deref(), Some("ABCDEF+Demo"));
            }
        }
        drop(registered);
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(doc.try_page_count().unwrap(), 2);
    }

    pub(crate) fn fixture() -> Vec<u8> {
        fixture_with_composite(false)
    }

    fn fixture_with_composite(composite: bool) -> Vec<u8> {
        let mut head = vec![0u8; 54];
        head[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5f0f3cf5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        head[40..42].copy_from_slice(&30i16.to_be_bytes());
        head[42..44].copy_from_slice(&60i16.to_be_bytes());
        head[50..52].copy_from_slice(&1i16.to_be_bytes());
        let mut hhea = vec![0; 36];
        hhea[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
        hhea[34..36].copy_from_slice(&3u16.to_be_bytes());
        let mut maxp = vec![0; 32];
        maxp[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        maxp[4..6].copy_from_slice(&3u16.to_be_bytes());
        maxp[6..8].copy_from_slice(&3u16.to_be_bytes());
        maxp[8..10].copy_from_slice(&1u16.to_be_bytes());
        maxp[14..16].copy_from_slice(&1u16.to_be_bytes());
        if composite {
            for (offset, value) in [(10usize, 3u16), (12, 1), (28, 1), (30, 1)] {
                maxp[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
            }
        }
        let mut post = vec![0; 32];
        post[..4].copy_from_slice(&0x00030000u32.to_be_bytes());
        let mut names = vec![0, 0, 0, 1, 0, 18];
        for value in [3u16, 1, 0x409, 6, 8, 0] {
            names.extend_from_slice(&value.to_be_bytes());
        }
        names.extend_from_slice(&[0, b'D', 0, b'e', 0, b'm', 0, b'o']);
        let mut cmap = vec![0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, 12, 0, 12, 0, 0];
        for value in [40u32, 0, 2, 65, 65, 1, 66, 66, 2] {
            cmap.extend_from_slice(&value.to_be_bytes());
        }
        let triangle = [
            0, 1, 0, 0, 0, 0, 0, 30, 0, 60, 0, 2, 0, 0, 0x3f, 2, 10, 10, 10, 20, 20, 20,
        ];
        let mut glyf = if composite {
            // One XY-positioned component referencing original GID 2.
            vec![255, 255, 0, 0, 0, 0, 0, 30, 0, 60, 0, 2, 0, 2, 0, 0]
        } else {
            triangle.to_vec()
        };
        let component_end = glyf.len() as u32;
        glyf.extend_from_slice(&triangle);
        let mut loca = Vec::new();
        for value in [0u32, 0, component_end, glyf.len() as u32] {
            loca.extend_from_slice(&value.to_be_bytes());
        }
        let mut hmtx = Vec::new();
        for _ in 0..3 {
            hmtx.extend_from_slice(&[1, 244, 0, 0]);
        }
        let os2 = vec![0; 78];
        let tables = [
            (*b"head", head.as_slice()),
            (*b"hhea", &hhea),
            (*b"hmtx", &hmtx),
            (*b"maxp", &maxp),
            (*b"post", &post),
            (*b"name", &names),
            (*b"cmap", &cmap),
            (*b"glyf", &glyf),
            (*b"loca", &loca),
            (*b"OS/2", &os2),
        ];
        write_sfnt(&tables, &Limits::default(), &OperationBudget::new(65536))
            .unwrap()
            .as_slice()
            .to_vec()
    }

    #[test]
    fn public_subset_table_policy_is_explicit_and_rejections_are_atomic() {
        use crate::document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions};
        let original = fixture();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&original)).unwrap();
        for (tag, omitted_without_opt_in) in [
            (*b"ZZZZ", false), (*b"fvar", false), (*b"gvar", false),
            (*b"COLR", false), (*b"GSUB", false), (*b"DSIG", true),
        ] {
            let tables = directory.table_records().map(|record| {
                let start = record.offset() as usize;
                (record.tag().as_bytes().try_into().unwrap(),
                    &original[start..start + record.length() as usize])
            }).chain(std::iter::once((tag, &[0u8; 4][..]))).collect::<Vec<_>>();
            let pool = OperationBudget::new(65536);
            let source = write_sfnt(&tables, &Limits::default(), &pool).unwrap();
            for omit in [false, true] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page(0).unwrap();
                let before = doc.save_to_bytes().unwrap();
                let baseline = pool.live_bytes();
                let result = doc.resources().embed_truetype_subset_font(
                    0, source.as_slice(), "A", TrueTypeEmbeddingOptions {
                        postscript_name: "Demo".into(), cap_height: 60.0,
                        stem_v: 80.0, serif: false,
                    }, *b"ABCDEF", omit, &pool,
                );
                if omitted_without_opt_in || (tag == *b"GSUB" && omit) {
                    let font = result.unwrap();
                    assert_eq!(font.omitted_tables(), &[tag]);
                    drop(font);
                } else {
                    assert!(result.is_err(), "tag {tag:?}, omit {omit}");
                    assert_eq!(doc.save_to_bytes().unwrap(), before);
                }
                assert_eq!(pool.live_bytes(), baseline, "tag {tag:?}, omit {omit}");
            }
            drop(source);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn shared_gid_non_bmp_subset_survives_assembly_and_public_edit() {
        use crate::{document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
            subset_text_edit::SubsetTextEditOptions}, parser::syntax::ParseMode};
        let original_source = fixture();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&original_source)).unwrap();
        let mut cmap = vec![0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, 12, 0, 12, 0, 0];
        for value in [52u32, 0, 3, 65, 65, 1, 66, 66, 1, 0x20000, 0x20000, 1] {
            cmap.extend_from_slice(&value.to_be_bytes());
        }
        let tables = directory.table_records().map(|record| {
            let tag: [u8; 4] = record.tag().as_bytes().try_into().unwrap();
            let start = record.offset() as usize;
            let bytes = if tag == *b"cmap" { cmap.as_slice() } else {
                &original_source[start..start + record.length() as usize]
            };
            (tag, bytes)
        }).collect::<Vec<_>>();
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let source = write_sfnt(&tables, &Limits::default(), &pool).unwrap();
        let program = TrueTypeProgram::try_new(source.as_slice()).unwrap();
        let encoding = UnicodeCidEncoding::from_text(&program, "A𠀀BA", Limits::default()).unwrap();
        assert_eq!(encoding.cid_to_gid_map().unwrap(), [0, 0, 0, 1, 0, 1, 0, 1]);
        let subset = build_subset(source.as_slice(), &encoding, "ABCDEF+Demo", false,
            &Limits::default(), &pool).unwrap();
        let rebuilt = TrueTypeProgram::try_new(subset.bytes.as_slice()).unwrap();
        for scalar in ['A', 'B', '𠀀'] {
            assert_eq!(rebuilt.unicode_glyph_id(scalar).unwrap(), Some(1));
        }
        let locations = TrueTypeProgram::checked_glyph_locations(subset.bytes.as_slice(), Limits::default()).unwrap();
        assert!(!locations.glyph_data(1).unwrap().is_empty());
        assert!(locations.glyph_data(2).unwrap().is_empty());
        assert_eq!(super::super::sfnt::table_checksum(subset.bytes.as_slice()), 0xB1B0AFBA);
        drop(subset);
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let embedding = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(), cap_height: 60.0, stem_v: 80.0, serif: false,
        };
        let font = doc.resources().embed_truetype_subset_font(0, source.as_slice(), "A",
            embedding(), *b"AAAAAA", false, &pool).unwrap();
        let second = font.font.register_on_page(&doc, 1).unwrap();
        for (page, name) in [(0, font.font.resource_name.as_str()), (1, second.as_str())] {
            doc.replace_page_contents(page, format!("BT /{name} 12 Tf <00010001> Tj ET").into_bytes()).unwrap();
        }
        let baseline = pool.live_bytes();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        doc.replace_page_text_string_with_subset_and_budget(0, &root, 2, 2..2, "B𠀀",
            SubsetTextEditOptions {
                source: source.as_slice(), embedding: embedding(), subset_tag: *b"BBBBBB",
                omit_layout_and_auxiliary: false, preserve_advance: true,
            }, &pool).unwrap();
        assert_eq!(pool.live_bytes(), baseline);
        assert!(font.font.encoding().encode_text("B𠀀").is_err());
        let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        for (page, expected) in [(0, "AB𠀀A"), (1, "AA")] {
            let text = reopened.get_page(page).unwrap().extract_characters_paint_order().unwrap()
                .iter().filter_map(|ch| ch.unicode.as_deref()).collect::<String>();
            assert_eq!(text, expected);
        }
        drop(font);
        drop(source);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn assembled_composite_keeps_unencoded_component_and_original_reference() {
        let source = fixture_with_composite(true);
        let limits = Limits::default();
        let program = TrueTypeProgram::try_new(&source).unwrap();
        let encoding = UnicodeCidEncoding::from_text(&program, "A", limits).unwrap();
        let budget = OperationBudget::new(65536);
        let result =
            build_subset(&source, &encoding, "ABCDEF+Demo", false, &limits, &budget).unwrap();
        let reopened = TrueTypeProgram::try_new(result.bytes.as_slice()).unwrap();
        assert_eq!(reopened.unicode_glyph_id('B').unwrap(), None);
        let locations =
            TrueTypeProgram::checked_glyph_locations(result.bytes.as_slice(), limits).unwrap();
        let composite = locations.composite_glyph(1, &limits).unwrap().unwrap();
        assert_eq!(composite.components.len(), 1);
        assert_eq!(composite.components[0].glyph_id, 2);
        assert!(!locations.glyph_data(2).unwrap().is_empty());
        assert_eq!(
            locations
                .validate_composite_dependencies(1, &limits)
                .unwrap()
                .visited_glyphs,
            2
        );
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn assembled_subset_reopens_preserves_gid_and_releases_exact_peak_failure() {
        let mut source = fixture();
        let directory = Sfnt::try_new(&mut TrueTypeReader::new(&source)).unwrap();
        let head_offset = directory.get_table("head").unwrap().offset() as usize;
        for (slot, value) in source[head_offset + 36..head_offset + 44]
            .chunks_exact_mut(2)
            .zip([-200i16, -300, 400, 500])
        {
            slot.copy_from_slice(&value.to_be_bytes());
        }
        let original = source.clone();
        let limits = Limits::default();
        let program = TrueTypeProgram::try_new(&source).unwrap();
        let encoding = UnicodeCidEncoding::from_text(&program, "AAA", limits).unwrap();
        let budget = OperationBudget::new(65536);
        let result =
            build_subset(&source, &encoding, "ABCDEF+Demo", false, &limits, &budget).unwrap();
        assert!(result.omitted_tables.is_empty());
        let reopened = TrueTypeProgram::try_new(result.bytes.as_slice()).unwrap();
        assert_eq!(reopened.unicode_glyph_id('A').unwrap(), Some(1));
        assert_eq!(reopened.unicode_glyph_id('B').unwrap(), None);
        assert_eq!(reopened.num_glyphs(), Some(3));
        assert_eq!(
            program.horizontal_metrics().unwrap().bounds(),
            [-200, -300, 400, 500]
        );
        assert_eq!(
            reopened.horizontal_metrics().unwrap().bounds(),
            [0, 0, 30, 60]
        );
        let locations =
            TrueTypeProgram::checked_glyph_locations(result.bytes.as_slice(), limits).unwrap();
        assert!(locations.glyph_data(2).unwrap().is_empty());
        assert!(!locations.glyph_data(1).unwrap().is_empty());
        assert_eq!(
            super::super::sfnt::table_checksum(result.bytes.as_slice()),
            0xB1B0AFBA
        );
        assert_eq!(source, original);
        let peak = budget.peak_bytes();
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
        let tight = OperationBudget::new(peak + 6);
        let neighbor = tight.reserve(7).unwrap();
        assert!(
            matches!(build_subset(&source, &encoding, "ABCDEF+Demo", false, &limits, &tight),
            Err(PdfError::ResourceLimit(message)) if message == "operation live bytes")
        );
        assert_eq!(tight.live_bytes(), 7);
        drop(neighbor);
        drop(build_subset(&source, &encoding, "ABCDEF+Demo", false, &limits, &tight).unwrap());
        assert_eq!(tight.live_bytes(), 0);
    }
}
