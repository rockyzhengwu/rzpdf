//! Offline views of the actual Prev chain, never the mutable object overlay.
use super::*;
use crate::{document::PdfDocument, parser::options::ParseOptions};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureField {
    pub object_id: ObjectId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureObjectState {
    Unchanged,
    Replaced,
    Removed,
}

struct Revision {
    xref: u64,
    eof: usize,
    end: usize,
}

fn whitespace(byte: u8) -> bool {
    matches!(byte, 0 | 9 | 10 | 12 | 13 | 32)
}

// Read the terminator immediately following the parsed xref section. Searching
// globally for %%EOF would accept markers embedded in streams or strings.
fn revisions(bytes: &[u8], options: ParseOptions) -> PdfResult<Vec<Revision>> {
    let mut parser =
        SyntaxParser::with_options(StreamReader::try_new(Cursor::new(bytes))?, options);
    parser.read_header()?;
    let (_, mut offset) = crate::parser::parser::parse_xref_with_offset(&mut parser)?;
    let mut result = Vec::new();
    loop {
        if result.len() >= options.limits.max_depth {
            return Err(PdfError::ResourceLimit("signature revision count".into()));
        }
        parser.set_pos(offset)?;
        let table = if parser.peek_word()?.is_equal(b"xref") {
            crate::parser::parser::parse_xref_section(&mut parser, offset)?
                .ok_or_else(|| malformed("historical xref table"))?
        } else {
            crate::parser::parser::parse_xref_stream(&mut parser, offset)?
        };
        if table.has_hybrid_references() {
            return Err(invalid("hybrid signature history unsupported"));
        }
        if table
            .trailer()
            .get("Encrypt")
            .is_some_and(|v| !matches!(v, PdfObject::PdfNull))
        {
            return Err(invalid("encrypted signature history unsupported"));
        }
        if !parser.get_next_word()?.is_equal(b"startxref")
            || parser.get_next_word()?.as_u64_checked()? != offset
        {
            return Err(malformed("xref section is not followed by its startxref"));
        }
        let mut end = parser.position() as usize;
        while bytes.get(end).copied().is_some_and(whitespace) {
            end += 1;
        }
        if bytes.get(end..end.saturating_add(5)) != Some(b"%%EOF") {
            return Err(malformed("historical revision EOF marker"));
        }
        end += 5;
        let eof = end;
        if bytes.get(end).copied().is_some_and(|b| !whitespace(b)) {
            return Err(malformed("historical EOF marker delimiter"));
        }
        while bytes.get(end).copied().is_some_and(whitespace) {
            end += 1;
        }
        if result.is_empty() && end != bytes.len() {
            return Err(malformed("data after latest revision EOF"));
        }
        if result
            .last()
            .is_some_and(|newer: &Revision| end > newer.xref as usize)
        {
            return Err(malformed("crossing revision boundaries"));
        }
        result.push(Revision {
            xref: offset,
            eof,
            end,
        });
        let previous = table
            .trailer()
            .get("Prev")
            .map(|v| {
                v.as_number()
                    .ok_or_else(|| malformed("Prev integer required"))?
                    .as_u64_checked()
            })
            .transpose()?
            .filter(|v| *v != 0);
        let Some(previous) = previous else {
            break;
        };
        if previous >= offset {
            return Err(malformed("Prev must point to an earlier revision"));
        }
        offset = previous;
    }
    result.reverse();
    Ok(result)
}

pub(super) fn revision_ends(bytes: &[u8], options: ParseOptions) -> PdfResult<Vec<usize>> {
    Ok(revisions(bytes, options)?.into_iter().map(|revision| revision.end).collect())
}

fn signature_range(dict: &PdfDict) -> PdfResult<[u64; 4]> {
    let array = dict
        .get("ByteRange")
        .and_then(PdfObject::as_array)
        .filter(|a| a.len() == 4)
        .ok_or_else(|| malformed("ByteRange requires four direct integers"))?;
    let mut range = [0; 4];
    for (i, value) in array.into_iter().enumerate() {
        range[i] = value
            .as_number()
            .ok_or_else(|| malformed("ByteRange integer"))?
            .as_u64_checked()?;
    }
    if range[0] != 0 || range[1] >= range[2] {
        return Err(malformed("overlapping or nonzero-start ByteRange"));
    }
    Ok(range)
}

fn fields<R: std::io::Read + std::io::Seek>(
    doc: &PdfDocument<R>,
    budget: &OperationBudget,
) -> PdfResult<HashMap<ObjectId, SignatureField>> {
    let mut result = HashMap::new();
    let trailer = doc.ctx().trailer();
    let root = trailer
        .get("Root")
        .ok_or_else(|| malformed("Root missing"))?;
    let (root, _root_lease) = doc.ctx().resolve_owned_with_budget(root, budget)?;
    drop(trailer);
    let root = root.as_dict().ok_or_else(|| malformed("Root dictionary"))?;
    let Some(form) = root.get("AcroForm") else {
        return Ok(result);
    };
    let (form, _form_lease) = doc.ctx().resolve_owned_with_budget(form, budget)?;
    let form = form
        .as_dict()
        .ok_or_else(|| malformed("AcroForm dictionary"))?;
    let Some(values) = form.get("Fields") else {
        return Ok(result);
    };
    let (values, _values_lease) = doc.ctx().resolve_owned_with_budget(values, budget)?;
    let values = values.as_array().ok_or_else(|| malformed("Fields array"))?;
    // Keep per-node reservations until the field-name/ownership tables drop.
    // Includes two bounded qualified-name copies and table/lease-vector slack.
    let mut field_leases = Vec::new();
    let mut seen = HashSet::new();
    let mut names = HashSet::new();
    for value in values {
        field(
            doc,
            value,
            None,
            "",
            None,
            0,
            budget,
            &mut field_leases,
            &mut seen,
            &mut names,
            &mut result,
        )?;
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn field<R: std::io::Read + std::io::Seek>(
    doc: &PdfDocument<R>,
    value: &PdfObject,
    parent: Option<ObjectId>,
    prefix: &str,
    inherited_type: Option<&str>,
    depth: usize,
    budget: &OperationBudget,
    field_leases: &mut Vec<crate::budget::MemoryLease>,
    seen: &mut HashSet<ObjectId>,
    names: &mut HashSet<String>,
    result: &mut HashMap<ObjectId, SignatureField>,
) -> PdfResult<()> {
    if depth >= doc.ctx().limits().max_depth || seen.len() >= doc.ctx().limits().max_objects {
        return Err(PdfError::ResourceLimit("signature field tree".into()));
    }
    let id = value
        .as_object_id()
        .ok_or_else(|| invalid("direct signature field ownership unsupported"))?;
    field_leases.push(budget.reserve(16 * 1024)?);
    if !seen.insert(id) {
        return Err(malformed("shared or cyclic field ownership"));
    }
    let (object, _lease) = doc.ctx().resolve_owned_with_budget(value, budget)?;
    let dict = object
        .as_dict()
        .ok_or_else(|| malformed("field dictionary"))?;
    if dict.get("Parent").and_then(PdfObject::as_object_id) != parent {
        return Err(malformed("field Parent does not match ownership"));
    }
    let mut name = prefix.to_owned();
    if let Some(part) = dict.get("T") {
        let part = part
            .as_string()
            .ok_or_else(|| malformed("field T string"))?;
        if part.bytes().len() > 8194 {
            return Err(PdfError::ResourceLimit(
                "signature partial field name bytes".into(),
            ));
        }
        let _text_lease = budget.reserve(part.bytes().len().saturating_mul(4))?;
        let part = part.text_content_with_limits(&doc.ctx().limits())?;
        if part.is_empty() || part.contains('.') || part.chars().any(char::is_control) {
            return Err(malformed("invalid partial field name"));
        }
        if !name.is_empty() {
            name.push('.');
        }
        name.push_str(&part);
        if name.len() > 4096 || !names.insert(name.clone()) {
            return Err(malformed("duplicate or excessive qualified field name"));
        }
    }
    let ft = dict
        .get("FT")
        .map(|v| {
            v.as_name()
                .map(|v| v.name())
                .ok_or_else(|| malformed("field FT name"))
        })
        .transpose()?
        .or(inherited_type);
    // A pure Widget child is not a second field binding to the inherited V.
    if ft == Some("Sig")
        && let Some(v) = dict.get("V").filter(|v| !matches!(v, PdfObject::PdfNull))
    {
        if result.len() >= 16 {
            return Err(PdfError::ResourceLimit("signature field bindings".into()));
        }
        let sig = v
            .as_object_id()
            .ok_or_else(|| invalid("direct signature value unsupported"))?;
        if result
            .insert(
                sig,
                SignatureField {
                    object_id: id,
                    name: name.clone(),
                },
            )
            .is_some()
        {
            return Err(malformed("signature bound to multiple fields"));
        }
    }
    if let Some(kids) = dict.get("Kids") {
        let (kids, _kids_lease) = doc.ctx().resolve_owned_with_budget(kids, budget)?;
        for kid in kids
            .as_array()
            .ok_or_else(|| malformed("field Kids array"))?
        {
            field(
                doc,
                kid,
                Some(id),
                &name,
                ft,
                depth + 1,
                budget,
                field_leases,
                seen,
                names,
                result,
            )?;
        }
    }
    Ok(())
}

pub(super) fn reports(
    doc: &Document,
    budget: &OperationBudget,
    verify: bool,
) -> PdfResult<Vec<SignatureReport>> {
    let Some(bytes) = doc.signature_source()? else {
        return Ok(Vec::new());
    };
    let options = doc.ctx().options();
    // Conservative envelope for legacy parser/xref/field builders. The source
    // itself is borrowed, not cloned. Explicit object leases use the same pool.
    let _envelope = budget.reserve(
        bytes
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(1024 * 1024))
            .ok_or_else(|| PdfError::ResourceLimit("signature history envelope".into()))?,
    )?;
    let revisions = revisions(bytes, options)?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut work = 0usize;
    for (index, revision) in revisions.iter().enumerate() {
        let view =
            PdfDocument::from_reader_with_options(Cursor::new(&bytes[..revision.end]), options)?;
        let mut bindings = None;
        for info in view.ctx().source_xref().objects().values() {
            work += 1;
            if work > options.limits.max_objects {
                return Err(PdfError::ResourceLimit(
                    "signature history object visits".into(),
                ));
            }
            if matches!(info.state(), ObjectState::Free) {
                continue;
            }
            let (object, _lease) = view
                .ctx()
                .resolve_source_owned_with_budget(info.object_id(), budget)?;
            let Some(dict) = object.as_dict().filter(|d| d.contains_key("ByteRange")) else {
                continue;
            };
            if !matches!(info.state(), ObjectState::Normal) {
                return Err(invalid("compressed signature dictionary unsupported"));
            }
            if !seen.insert((info.object_id(), info.offset())) {
                continue;
            }
            if result.len() >= 16 {
                return Err(PdfError::ResourceLimit("signature report count".into()));
            }
            let range = signature_range(dict)?;
            let end = usize::try_from(
                range[2]
                    .checked_add(range[3])
                    .ok_or_else(|| malformed("ByteRange overflow"))?,
            )
            .map_err(|_| malformed("ByteRange exceeds address space"))?;
            if end < revision.eof || end > revision.end {
                return Err(malformed(
                    "signature does not cover the revision where it first appears",
                ));
            }
            let locations = signature_locations_with_budget(
                &bytes[..end],
                info.object_id(),
                options.limits,
                budget,
            )?;
            if range[1] != locations.contents.start as u64
                || range[2] != locations.contents.end as u64
            {
                return Err(malformed(
                    "ByteRange exclusion must be exactly this signature Contents",
                ));
            }
            // Unsigned CMS bytes must not also act as another live PDF object
            // or part of a live stream/string. Otherwise changing Contents
            // could change the interpreted document without changing its hash.
            for other in view.ctx().source_xref().objects().values() {
                if !matches!(other.state(), ObjectState::Normal)
                    || other.object_id() == info.object_id()
                {
                    continue;
                }
                work += 1;
                if work > options.limits.max_objects {
                    return Err(PdfError::ResourceLimit(
                        "signature object extent visits".into(),
                    ));
                }
                if other.offset() >= range[2] {
                    continue;
                }
                if other.offset() >= range[1]
                    || view.ctx().source_object_end_with_budget(
                        other.offset(),
                        other.object_id(),
                        budget,
                    )? > range[1]
                {
                    return Err(malformed(
                        "active object overlaps excluded signature Contents",
                    ));
                }
            }
            let cms = dict
                .get("Contents")
                .and_then(PdfObject::as_string)
                .ok_or_else(|| malformed("direct Contents string required"))?;
            let verification = if verify {
                if dict
                    .get("SubFilter")
                    .and_then(PdfObject::as_name)
                    .is_some_and(|v| v.name() == "adbe.pkcs7.detached")
                {
                    verify_cms(
                        cms.bytes(),
                        bytes,
                        range,
                        budget,
                        view.effective_version_with_budget(budget)?
                            >= crate::document::save_options::PdfVersion::V2_0,
                    )?
                } else {
                    SignatureVerification::Unsupported
                }
            } else {
                SignatureVerification::NotPerformed
            };
            if bindings.is_none() {
                bindings = Some(fields(&view, budget)?);
            }
            let state = match doc.ctx().source_xref().lookup_id(info.object_id()) {
                Some(latest) if matches!(latest.state(), ObjectState::Free) => {
                    SignatureObjectState::Removed
                }
                Some(latest)
                    if matches!(latest.state(), ObjectState::Normal)
                        && latest.offset() == info.offset() =>
                {
                    SignatureObjectState::Unchanged
                }
                Some(_) => SignatureObjectState::Replaced,
                None => SignatureObjectState::Removed,
            };
            result.push(SignatureReport {
                object_id: info.object_id(),
                byte_range: range,
                signed_revision_bytes: end as u64,
                covers_current_document: end == bytes.len(),
                verification,
                signed_revision: index,
                signed_revision_xref: revision.xref,
                field: bindings.as_ref().unwrap().get(&info.object_id()).cloned(),
                source_object_state: state,
            });
        }
    }
    result.sort_by_key(|r| {
        (
            r.signed_revision,
            r.object_id.obj_num(),
            r.object_id.gen_num(),
        )
    });
    Ok(result)
}

fn verify_cms(
    padded: &[u8],
    bytes: &[u8],
    range: [u64; 4],
    budget: &OperationBudget,
    extended: bool,
) -> PdfResult<SignatureVerification> {
    // DER length, not trimming trailing zeroes (which can be valid signature bytes).
    let Some(length) = der_length(padded) else {
        return Ok(SignatureVerification::Malformed);
    };
    if length > 65536 {
        return Ok(SignatureVerification::Unsupported);
    }
    if padded[length..].iter().any(|b| *b != 0) {
        return Ok(SignatureVerification::Malformed);
    }
    let mut hash = Sha256::new();
    hash.update(&bytes[..range[1] as usize]);
    hash.update(&bytes[range[2] as usize..(range[2] + range[3]) as usize]);
    let digest: [u8; 32] = hash.finalize().into();
    match validate_cms(&padded[..length], &digest, budget, extended) {
        Ok(()) => Ok(SignatureVerification::Valid),
        Err(PdfError::SignatureMismatch(_)) => Ok(SignatureVerification::Invalid),
        Err(PdfError::MalformedSignature(_)) => Ok(SignatureVerification::Malformed),
        Err(PdfError::UnsupportedFeature(_)) => Ok(SignatureVerification::Unsupported),
        Err(error) => Err(error),
    }
}

fn der_length(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&0x30) {
        return None;
    }
    let first = *bytes.get(1)?;
    let (header, length) = if first < 128 {
        (2, first as usize)
    } else {
        let count = (first & 127) as usize;
        if count == 0 || count > 4 || *bytes.get(2)? == 0 {
            return None;
        }
        let mut n = 0usize;
        for b in bytes.get(2..2 + count)? {
            n = n.checked_mul(256)?.checked_add(*b as usize)?;
        }
        if n < 128 {
            return None;
        }
        (2 + count, n)
    };
    let end = length.checked_add(header)?;
    (end <= bytes.len()).then_some(end)
}
