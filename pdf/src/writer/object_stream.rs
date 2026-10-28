//! Full-save object packing. Logical identities stay in the document; these
//! containers and their offsets exist only for one serialization operation.
use super::*;
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    parser::options::Limits,
};

pub(super) const MAX_MEMBERS: usize = 100;
pub(super) const MAX_BYTES: usize = 1024 * 1024;
const CONTAINER: u8 = 1;
const STANDALONE: u8 = 2;

pub(super) struct Selection {
    flags: Vec<u8>,
    _lease: MemoryLease,
}

impl Selection {
    pub(super) fn inspect<R: std::io::Read + std::io::Seek>(
        doc: &PdfDocument<R>,
        ids: &[ObjectId],
        size: u32,
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        let mut lease = budget.reserve(size as usize)?;
        let mut flags = Vec::new();
        flags
            .try_reserve_exact(size as usize)
            .map_err(|_| PdfError::ResourceLimit("object-stream selection allocation".into()))?;
        lease.resize(flags.capacity())?;
        flags.resize(size as usize, 0);
        let mut result = Self {
            flags,
            _lease: lease,
        };
        for id in ids {
            let loaded = doc.ctx().object_for_write_with_budget(*id, budget)?;
            let Some((object, _)) = loaded.as_ref() else {
                continue;
            };
            if let PdfObject::PdfStream(stream) = object {
                let kind = stream
                    .dict()
                    .get("Type")
                    .and_then(PdfObject::as_name)
                    .map(|n| n.name());
                if matches!(kind, Some("ObjStm" | "XRef")) {
                    for (key, _) in stream.dict() {
                        let allowed = match kind {
                            Some("ObjStm") => matches!(
                                key.as_str(),
                                "Type" | "N" | "First" | "Length" | "Filter" | "DecodeParms" | "DL"
                            ),
                            _ => matches!(
                                key.as_str(),
                                "Type"
                                    | "Size"
                                    | "Index"
                                    | "W"
                                    | "Prev"
                                    | "Root"
                                    | "Info"
                                    | "Encrypt"
                                    | "ID"
                                    | "Length"
                                    | "Filter"
                                    | "DecodeParms"
                                    | "XRefStm"
                                    | "DL"
                            ),
                        };
                        if !allowed {
                            return Err(PdfError::UnsupportedFeature(
                                "object-stream repacking cannot discard Extends or unknown container fields".into()));
                        }
                    }
                    result.flags[id.obj_num() as usize] |= CONTAINER;
                }
                // Length indirection chains must stay outside object streams.
                let mut length = stream
                    .dict()
                    .get("Length")
                    .and_then(PdfObject::as_reference)
                    .map(|r| r.id());
                let mut depth = 0;
                while let Some(target) = length {
                    if depth > doc.ctx().limits().max_depth {
                        return Err(PdfError::ResourceLimit(
                            "stream Length reference depth".into(),
                        ));
                    }
                    let Some(flag) = result.flags.get_mut(target.obj_num() as usize) else {
                        break;
                    };
                    if *flag & STANDALONE != 0 {
                        break;
                    }
                    *flag |= STANDALONE;
                    let loaded = doc.ctx().object_for_write_with_budget(target, budget)?;
                    length = loaded
                        .as_ref()
                        .and_then(|(o, _)| o.as_reference())
                        .map(|r| r.id());
                    depth += 1;
                }
            }
        }
        let trailer = doc.ctx().trailer();
        for key in ["Root", "Encrypt"] {
            if let Some(reference) = trailer.get(key).and_then(PdfObject::as_reference)
                && let Some(flag) = result.flags.get_mut(reference.id().obj_num() as usize)
            {
                *flag |= STANDALONE;
            }
        }
        // Old offsets and compressed membership are rebuilt, not copied.
        // If an ordinary owner references a container it has semantic identity;
        // refusing is safer than silently removing that identity or stale bytes.
        for id in ids {
            if result.removed(*id) {
                continue;
            }
            let loaded = doc.ctx().object_for_write_with_budget(*id, budget)?;
            if let Some((object, _)) = loaded.as_ref() {
                visit_references(object, doc.ctx().limits().max_depth, 0, &mut |target| {
                    result.require_not_removed(target)
                })?;
            }
        }
        for (_, value) in trailer.into_iter().filter(|(k, _)| retained_trailer_key(k)) {
            visit_references(value, doc.ctx().limits().max_depth, 0, &mut |target| {
                result.require_not_removed(target)
            })?;
        }
        Ok(result)
    }

    fn require_not_removed(&self, id: ObjectId) -> PdfResult<()> {
        if self.removed(id) {
            return Err(PdfError::UnsupportedFeature(
                "active reference to a replaced structural container".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn removed(&self, id: ObjectId) -> bool {
        self.flags
            .get(id.obj_num() as usize)
            .is_some_and(|f| f & CONTAINER != 0)
    }

    pub(super) fn eligible(&self, id: ObjectId, object: &PdfObject) -> bool {
        if id.obj_num() == 0
            || id.gen_num() != 0
            || self
                .flags
                .get(id.obj_num() as usize)
                .is_none_or(|f| *f != 0)
            || matches!(object, PdfObject::PdfStream(_) | PdfObject::PdfReference(_))
        {
            return false;
        }
        !contains_barrier(object)
    }
}

// A direct nested signature/placeholder must not get packed merely because
// its enclosing indirect object is an array or an ordinary dictionary.
fn contains_barrier(object: &PdfObject) -> bool {
    match object {
        PdfObject::PdfDict(dict) => {
            // Catalog stays independently discoverable. Signature dictionaries,
            // placeholders and signature fields never enter a general packer.
            let kind = dict
                .get("Type")
                .and_then(PdfObject::as_name)
                .map(|n| n.name());
            if matches!(
                kind,
                Some("Catalog" | "Sig" | "DocTimeStamp" | "XRef" | "ObjStm")
            ) || dict.contains_key("ByteRange")
                || dict.contains_key("DocMDP")
                || dict.contains_key("FieldMDP")
                || dict.get("SigFlags").is_some_and(|v| v.as_u32() != Some(0))
                || dict.contains_key("Linearized")
                || dict
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .is_some_and(|n| n.name() == "Sig")
            {
                return true;
            }
            dict.into_iter().any(|(_, value)| contains_barrier(value))
        }
        PdfObject::PdfArray(array) => array.into_iter().any(contains_barrier),
        PdfObject::PdfStream(_) => true,
        _ => false,
    }
}

fn contains_strings(object: &PdfObject) -> bool {
    match object {
        PdfObject::PdfString(_) => true,
        PdfObject::PdfArray(a) => a.into_iter().any(contains_strings),
        PdfObject::PdfDict(d) => d.into_iter().any(|(_, v)| contains_strings(v)),
        _ => false,
    }
}

pub(super) fn visit_references(
    object: &PdfObject,
    max_depth: usize,
    depth: usize,
    visitor: &mut impl FnMut(ObjectId) -> PdfResult<()>,
) -> PdfResult<()> {
    if depth > max_depth {
        return Err(PdfError::ResourceLimit("packing reference depth".into()));
    }
    match object {
        PdfObject::PdfReference(r) => visitor(r.id())?,
        PdfObject::PdfArray(a) => {
            for item in a {
                visit_references(item, max_depth, depth + 1, visitor)?;
            }
        }
        PdfObject::PdfDict(d) => {
            for (_, item) in d {
                visit_references(item, max_depth, depth + 1, visitor)?;
            }
        }
        PdfObject::PdfStream(s) => {
            for (_, item) in s.dict() {
                visit_references(item, max_depth, depth + 1, visitor)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// No token buffers or dictionary sort tables are allocated for this size pass.
// Exactly mirror write_object_limited's whitespace and escaping rules.
fn member_length(object: &PdfObject, depth: usize, max_depth: usize) -> PdfResult<usize> {
    if depth > max_depth {
        return Err(PdfError::ResourceLimit("packing member depth".into()));
    }
    let add = |a: usize, b: usize| {
        a.checked_add(b)
            .ok_or_else(|| PdfError::ResourceLimit("object serialization length".into()))
    };
    Ok(match object {
        PdfObject::PdfNull => 4,
        PdfObject::PdfBool(b) => {
            if b.value() {
                4
            } else {
                5
            }
        }
        PdfObject::PdfNumber(n) => validated_number_lexeme(n)?.len(),
        PdfObject::PdfName(n) => serialized_name_length(n)?,
        PdfObject::PdfString(s) => serialized_string_length(s)?,
        PdfObject::PdfReference(r) => digits(r.objnum() as usize) + digits(r.gennum() as usize) + 3,
        PdfObject::PdfArray(a) => {
            let mut length = 2;
            for (index, value) in a.into_iter().enumerate() {
                length = add(length, usize::from(index > 0))?;
                length = add(length, member_length(value, depth + 1, max_depth)?)?;
            }
            length
        }
        PdfObject::PdfDict(d) => {
            let mut length = 5; // << and newline >>
            for (key, value) in d {
                length = add(length, 2)?; // newline and space
                length = add(length, serialized_name_bytes_length(key.as_bytes())?)?;
                length = add(length, member_length(value, depth + 1, max_depth)?)?;
            }
            length
        }
        PdfObject::PdfStream(_) => {
            return Err(PdfError::ObjectError(
                "stream cannot be an object-stream member".into(),
            ));
        }
    })
}

pub(super) struct Packer<'a> {
    members: [(ObjectId, usize); MAX_MEMBERS],
    count: usize,
    index_length: usize,
    body: SerializationOutput,
    maximum: usize,
    compression: bool,
    security: Option<&'a crate::security::SecurityHandler>,
}

fn digits(mut n: usize) -> usize {
    let mut count = 1;
    while n >= 10 {
        count += 1;
        n /= 10;
    }
    count
}

impl<'a> Packer<'a> {
    pub(super) fn new(
        limits: Limits,
        budget: &OperationBudget,
        compression: bool,
        security: Option<&'a crate::security::SecurityHandler>,
    ) -> PdfResult<Self> {
        // Leave room for the IV and mandatory padding even without Flate.
        // This conservative grouping cap avoids creating a raw group that
        // fits before encryption but cannot fit its ciphertext afterwards.
        let raw_cap = if security.is_some_and(|s| s.report().streams_use_aes256) {
            (limits.max_raw_stream_bytes.saturating_sub(16) / 16 * 16).saturating_sub(1)
        } else {
            limits.max_raw_stream_bytes
        };
        let maximum = MAX_BYTES.min(raw_cap).min(limits.max_decoded_bytes);
        Ok(Self {
            members: [(ObjectId::new(0, 0), 0); MAX_MEMBERS],
            count: 0,
            index_length: 0,
            body: SerializationOutput {
                bytes: BudgetedBytes::copy_from_slice(budget, &[])?,
                limits,
                capacity_limit: maximum,
            },
            maximum,
            compression,
            security,
        })
    }

    pub(super) fn try_pack(
        &mut self,
        id: ObjectId,
        object: &PdfObject,
        plan: &mut layout::FullLayoutPlan,
        output: &mut impl OutputBuffer,
        limit: usize,
    ) -> PdfResult<bool> {
        if self
            .security
            .is_some_and(|s| s.report().strings_use_aes256 != s.report().streams_use_aes256)
            && contains_strings(object)
        {
            return Ok(false);
        }
        let body_length = member_length(object, 0, self.body.limits.max_depth)?
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("object member length".into()))?;
        let single_index = digits(id.obj_num() as usize) + 3; // "id 0\n"
        if body_length
            .checked_add(single_index)
            .is_none_or(|n| n > self.maximum)
        {
            return Ok(false);
        }
        let index_delta = digits(id.obj_num() as usize) + digits(self.body.len()) + 2;
        if self.count == MAX_MEMBERS
            || self.body.len() + body_length + self.index_length + index_delta > self.maximum
        {
            self.flush(plan, output, limit)?;
        }
        self.members[self.count] = (id, self.body.len());
        self.index_length += digits(id.obj_num() as usize) + digits(self.body.len()) + 2;
        write_object_limited(&mut self.body, object, self.maximum)?;
        append_output(&mut self.body, b"\n", self.maximum)?;
        self.count += 1;
        Ok(true)
    }

    pub(super) fn flush(
        &mut self,
        plan: &mut layout::FullLayoutPlan,
        output: &mut impl OutputBuffer,
        limit: usize,
    ) -> PdfResult<()> {
        if self.count == 0 {
            return Ok(());
        }
        let budget = self.body.bytes.operation_budget();
        let limits = self.body.limits;
        let mut payload = SerializationOutput {
            bytes: BudgetedBytes::copy_from_slice(&budget, &[])?,
            limits,
            capacity_limit: self.maximum,
        };
        for (id, offset) in &self.members[..self.count] {
            append_formatted_output(
                &mut payload,
                format_args!("{} {offset}\n", id.obj_num()),
                self.maximum,
            )?;
        }
        let first = payload.len();
        debug_assert_eq!(first, self.index_length);
        append_output(&mut payload, self.body.bytes.as_slice(), self.maximum)?;
        let compressed = if self.compression {
            // A bounded compression trial may be slightly larger than raw data.
            // Keep raw payload if encoding would exceed the stream-byte limit.
            let mut trial_limits = limits;
            trial_limits.max_raw_stream_bytes = self
                .maximum
                .checked_mul(2)
                .ok_or_else(|| PdfError::ResourceLimit("object-stream compression bound".into()))?;
            Some(compress_with_budget(
                payload.bytes.as_slice(),
                &budget,
                trial_limits,
            )?)
        } else {
            None
        };
        let encoded = compressed.as_ref().filter(|b| {
            b.as_slice().len() < payload.len() && b.as_slice().len() <= limits.max_raw_stream_bytes
        });
        let bytes = encoded
            .map(|b| b.as_slice())
            .unwrap_or(payload.bytes.as_slice());
        let ciphertext = self
            .security
            .map(|handler| {
                handler.encrypt_object_stream_bytes(bytes, limits.max_raw_stream_bytes, &budget)
            })
            .transpose()?;
        let bytes = ciphertext.as_ref().map(|b| b.as_slice()).unwrap_or(bytes);
        let token_bytes = 6usize
            .max(digits(first))
            .max(digits(bytes.len()))
            .max(digits(self.count))
            .max(
                self.members[..self.count]
                    .iter()
                    .map(|(id, offset)| digits(id.obj_num() as usize).max(digits(*offset)))
                    .max()
                    .unwrap_or(1),
            )
            .max(if encoded.is_some() {
                "FlateDecode".len()
            } else {
                0
            });
        if token_bytes > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit(
                "object-stream descriptor/index token bytes".into(),
            ));
        }
        let container = plan.allocate_container(limits, &budget)?;
        plan.record(container, output.len() as u64)?;
        append_formatted_output(
            output,
            format_args!("{} 0 obj\n", container.obj_num()),
            limit,
        )?;
        append_output(output, b"<< /Type /ObjStm", limit)?;
        append_formatted_output(
            output,
            format_args!(" /N {} /First {first}", self.count),
            limit,
        )?;
        append_formatted_output(output, format_args!(" /Length {}", bytes.len()), limit)?;
        if encoded.is_some() {
            append_output(output, b" /Filter /FlateDecode", limit)?;
        }
        append_output(output, b" >>\nstream\n", limit)?;
        append_output(output, bytes, limit)?;
        append_output(output, b"\nendstream\nendobj\n", limit)?;
        for (index, (id, _)) in self.members[..self.count].iter().enumerate() {
            plan.record_compressed(*id, container, index as u32)?;
        }
        self.body.bytes = BudgetedBytes::copy_from_slice(&budget, &[])?;
        self.count = 0;
        self.index_length = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::stream_reader::StreamReader,
        parser::syntax::{ParseMode, SyntaxParser},
    };
    use std::io::Cursor;

    #[test]
    fn allocation_free_member_size_matches_writer_escaping_and_whitespace() {
        for syntax in [
            b"null ".as_slice(),
            b"true ",
            b"false ",
            b"12345678901234567890 ",
            b"/A#20B#FF ",
            b"(A\\n\\r\\t\\b\\f\\(\\)\\\\Z) ",
            b"<00ff2841> ",
            b"[[] <<>> 23 4 R false] ",
            b"<< /A [ /B (escaped\\nvalue) 3 2 R ] /X#20Y << /Z -0.0123 >> >> ",
        ] {
            let mut parser = SyntaxParser::with_mode(
                StreamReader::try_new(Cursor::new(syntax)).unwrap(),
                ParseMode::Strict,
            );
            let object = parser.get_object().unwrap();
            let mut output = Vec::new();
            write_object_limited(&mut output, &object, usize::MAX).unwrap();
            assert_eq!(member_length(&object, 0, 128).unwrap(), output.len());
        }
    }

    #[test]
    fn signature_catalog_and_structural_placeholders_never_enter_packer() {
        let budget = OperationBudget::new(1024);
        let selection = Selection {
            flags: vec![0; 5],
            _lease: budget.reserve(5).unwrap(),
        };
        for syntax in [
            b"<< /Type /Catalog >> ".as_slice(),
            b"<< /Type /Sig >> ",
            b"<< /Type /DocTimeStamp >> ",
            b"<< /Type /ObjStm >> ",
            b"<< /Type /XRef >> ",
            b"<< /ByteRange [] >> ",
            b"<< /Linearized 1 >> ",
            b"<< /FT /Sig >> ",
            b"3 0 R ",
            b"[ << /Type /Sig >> ] ",
            b"<< /Value << /ByteRange [] >> >> ",
        ] {
            let mut parser = SyntaxParser::with_mode(
                StreamReader::try_new(Cursor::new(syntax)).unwrap(),
                ParseMode::Strict,
            );
            let object = parser.get_object().unwrap();
            assert!(!selection.eligible(ObjectId::new(3, 0), &object));
        }
        assert!(selection.eligible(ObjectId::new(3, 0), &PdfObject::PdfNull));
        assert!(!selection.eligible(ObjectId::new(3, 1), &PdfObject::PdfNull));
    }
}
