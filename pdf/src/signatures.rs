//! Historical verification, bounded approval signing and controlled signed
//! updates on strict, immutable, unencrypted input.
//! The external backend signs the concatenation of `signed_ranges()` as detached
//! CMS, NOT the PDF digest as a raw RSA operation. Completion verifies SHA-256
//! signed attributes and bounded RSA PKCS#1/PSS or P-256 ECDSA, not certificate trust,
//! expiry, revocation, timestamps or PAdES conformance. Signed revision plans
//! separately enforce the supported DocMDP/FieldMDP and object-change profile.
mod algorithms;
pub(crate) mod append;
mod changes;
mod controlled;
mod fields;
mod history;
mod policy;
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    document::{
        Document, cross_ref_table::ObjectState, identity::DocumentStamp, page_tree::PageTree,
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
    },
    parser::{options::Limits, syntax::SyntaxParser},
};
pub use changes::{ModificationAuthorization, ModificationKind, RevisionModification};
pub use controlled::PreparedSignedUpdate;
pub use fields::{SignatureAppearance, SignatureFieldOptions};
pub use history::{SignatureField, SignatureObjectState};
pub use policy::{DocMdpPermission, FieldLockAction, FieldLockRule, SignatureConstraints};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    io::{Cursor, Write},
    ops::Range,
};

fn invalid(message: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("signature: {message}"))
}

fn malformed(message: &str) -> PdfError {
    PdfError::MalformedSignature(message.into())
}

pub(crate) fn has_signature(object: &PdfObject, limits: Limits, depth: usize) -> PdfResult<bool> {
    if depth > limits.max_depth {
        return Err(PdfError::ResourceLimit("signature inspection depth".into()));
    }
    let dict = object
        .as_dict()
        .or_else(|| object.as_stream().map(|s| s.dict()));
    if let Some(dict) = dict {
        if dict.contains_key("ByteRange")
            || dict.contains_key("DocMDP")
            || dict.contains_key("FieldMDP")
            || dict.get("SigFlags").is_some_and(|v| v.as_u32() != Some(0))
            || ["FT", "Type"].iter().any(|k| {
                dict.get(k)
                    .and_then(PdfObject::as_name)
                    .is_some_and(|n| n.name() == "Sig")
            })
        {
            return Ok(true);
        }
        for (_, child) in dict {
            if has_signature(child, limits, depth + 1)? {
                return Ok(true);
            }
        }
    } else if let Some(array) = object.as_array() {
        for child in array {
            if has_signature(child, limits, depth + 1)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[derive(Debug, Clone)]
pub struct SignatureOptions {
    pub field_name: String,
    /// DER CMS capacity, default 16 KiB; bounded to 1 MiB and parser token limit.
    pub reserved_cms_bytes: usize,
}
impl Default for SignatureOptions {
    fn default() -> Self {
        Self {
            field_name: "Signature1".into(),
            reserved_cms_bytes: 16 * 1024,
        }
    }
}

/// Immutable, document-specific, single-use preparation. Failed completion is
/// retryable. The owned byte reservation remains charged for its whole lifetime.
pub struct PreparedSignature {
    bytes: BudgetedBytes,
    contents: Range<usize>,
    byte_range: [u64; 4],
    digest: [u8; 32],
    stamp: DocumentStamp,
    completed: Cell<bool>,
    append_proof: Option<append::AppendProof>,
    field_proof: Option<fields::FieldProof>,
}
impl std::fmt::Debug for PreparedSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSignature")
            .field("byte_range", &self.byte_range)
            .field("completed", &self.completed.get())
            .field("document_bytes", &"[REDACTED]")
            .finish()
    }
}
impl PreparedSignature {
    pub fn byte_range(&self) -> [u64; 4] {
        self.byte_range
    }
    pub fn sha256_digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn signed_ranges(&self) -> (&[u8], &[u8]) {
        let bytes = self.bytes.as_slice();
        (&bytes[..self.contents.start], &bytes[self.contents.end..])
    }
    /// A failing caller-owned writer can retain partial output; retry with a
    /// fresh writer. This does not consume the prepared result.
    pub fn write_signed_content(&self, writer: &mut impl Write) -> PdfResult<()> {
        let (a, b) = self.signed_ranges();
        writer.write_all(a)?;
        writer.write_all(b)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureVerification {
    NotPerformed,
    /// Digest and CMS signature match. Does NOT establish certificate trust.
    Valid,
    Invalid,
    Malformed,
    Unsupported,
}
/// Source-byte structural information only. This is NOT a validity or trust
/// verdict and excludes uncommitted edits in the document overlay.
#[derive(Debug, Clone)]
pub struct SignatureReport {
    pub object_id: ObjectId,
    pub byte_range: [u64; 4],
    pub signed_revision_bytes: u64,
    pub covers_current_document: bool,
    pub verification: SignatureVerification,
    /// Zero-based revision index, oldest first, in the actual Prev chain.
    pub signed_revision: usize,
    pub signed_revision_xref: u64,
    pub field: Option<SignatureField>,
    pub source_object_state: SignatureObjectState,
}

fn name(value: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(value.into()))
}
fn number(value: u64) -> PdfResult<PdfObject> {
    Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
        value.to_string().as_bytes(),
    )?))
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn resolve_dict(
    document: &Document,
    value: &PdfObject,
    budget: &OperationBudget,
) -> PdfResult<(PdfDict, crate::budget::MemoryLease)> {
    let (value, lease) = document.ctx().resolve_owned_with_budget(value, budget)?;
    Ok((
        value
            .into_dict()
            .ok_or_else(|| invalid("expected dictionary"))?,
        lease,
    ))
}
fn resolve_array(
    document: &Document,
    value: &PdfObject,
    budget: &OperationBudget,
) -> PdfResult<(PdfArray, crate::budget::MemoryLease)> {
    let (value, lease) = document.ctx().resolve_owned_with_budget(value, budget)?;
    Ok((
        value
            .into_array()
            .ok_or_else(|| invalid("expected array"))?,
        lease,
    ))
}

impl Document {
    pub fn prepare_signature(&self, options: SignatureOptions) -> PdfResult<PreparedSignature> {
        self.prepare_signature_with_budget(options, &self.ctx().limits().operation_budget())
    }

    pub fn prepare_signature_with_budget(
        &self,
        options: SignatureOptions,
        budget: &OperationBudget,
    ) -> PdfResult<PreparedSignature> {
        self.ctx().require_unsigned_source_with_budget(budget)?;
        if self.ctx().source_xref().has_hybrid_references() {
            return Err(invalid(
                "hybrid-reference history unsupported; full-save and reopen first",
            ));
        }
        if self.security_report().is_some() {
            return Err(invalid("encrypted input unsupported"));
        }
        let limits = self.ctx().limits();
        if options.field_name.is_empty()
            || options.field_name.contains('.')
            || options.field_name.len() > 1024
            || !(1024..=1024 * 1024).contains(&options.reserved_cms_bytes)
            || options
                .reserved_cms_bytes
                .checked_mul(2)
                .is_none_or(|v| v > limits.max_token_bytes)
        {
            return Err(invalid(
                "field name or CMS reservation outside supported bounds",
            ));
        }
        // The isolated snapshot is charged until its parser and source drop.
        let base = if self.signature_source()?.is_some() {
            if self.effective_version()? < crate::document::save_options::PdfVersion::V1_4 {
                return Err(invalid("upgrade old input with full save before signing"));
            }
            self.save_incremental_with_budget(Default::default(), budget)?
        } else {
            self.save_with_security_and_budget(
                crate::document::save_options::SaveOptions {
                    version: Some(crate::document::save_options::PdfVersion::V2_0),
                    ..Default::default()
                },
                crate::security::SaveSecurity::Preserve,
                budget,
            )?
        };
        let (base, _base_lease) = base.into_reserved_parts();
        let document = Document::from_bytes_with_options(
            base,
            crate::parser::options::ParseOptions {
                mode: crate::parser::syntax::ParseMode::Strict,
                limits,
            },
        )?;
        // Budget conservative envelope for temporary builders/legacy form
        // inspection. This is explicit tracked accounting, not process RSS.
        let _construction = budget.reserve(
            options
                .reserved_cms_bytes
                .checked_mul(8)
                .and_then(|v| v.checked_add(1024 * 1024))
                .ok_or_else(|| invalid("allocation overflow"))?,
        )?;
        let page_location = PageTree::page_at_with_budget(document.ctx(), 0, budget)?;
        let (first, _first_page_lease) = page_location.into_parts();
        let root_id = document
            .ctx()
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| invalid("indirect Root required"))?;
        let (mut catalog, _catalog_lease) = resolve_dict(&document, &reference(root_id), budget)?;
        let (mut form, _form_lease) = match catalog.get("AcroForm") {
            Some(value) => resolve_dict(&document, value, budget)?,
            None => (PdfDict::default(), budget.reserve(0)?),
        };
        if form.contains_key("XFA")
            || form
                .get("NeedAppearances")
                .and_then(PdfObject::as_bool)
                .is_some_and(|b| b.value())
        {
            return Err(invalid("XFA or NeedAppearances unsupported"));
        }
        let (mut fields, _fields_lease) = match form.get("Fields") {
            Some(value) => resolve_array(&document, value, budget)?,
            None => (PdfArray::default(), budget.reserve(0)?),
        };
        // Deliberately bounded flat-form preparation, not a generic field-tree
        // editor. Validate before constructing any new signature objects.
        if fields.len() > 256 {
            return Err(invalid("at most 256 existing flat fields supported"));
        }
        for value in &fields {
            if value.as_object_id().is_none() {
                return Err(invalid("existing fields must be indirect"));
            }
            let (field, _lease) = document.ctx().resolve_owned_with_budget(value, budget)?;
            let field = field
                .as_dict()
                .ok_or_else(|| invalid("existing field dictionary required"))?;
            if ["Kids", "Parent", "AA"]
                .iter()
                .any(|k| field.contains_key(k))
                || !field
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .is_some_and(|n| matches!(n.name(), "Tx" | "Btn"))
                || field
                    .get("Subtype")
                    .and_then(PdfObject::as_name)
                    .is_none_or(|n| n.name() != "Widget")
            {
                return Err(invalid("only flat text/checkbox Widget fields supported"));
            }
            if field
                .get("FT")
                .and_then(PdfObject::as_name)
                .is_some_and(|n| n.name() == "Btn")
                && field
                    .get("Ff")
                    .map(|v| v.as_u32().unwrap_or(u32::MAX))
                    .unwrap_or(0)
                    & ((1 << 15) | (1 << 16))
                    != 0
            {
                return Err(invalid("radio and pushbutton fields unsupported"));
            }
            let title = field
                .get("T")
                .and_then(PdfObject::as_string)
                .ok_or_else(|| invalid("field name string required"))?;
            let _text_lease = budget.reserve(
                title
                    .bytes()
                    .len()
                    .checked_mul(4)
                    .ok_or_else(|| invalid("field name allocation overflow"))?,
            )?;
            let title = title.text_content_with_limits(&limits)?;
            if title.contains('.') || title == options.field_name {
                return Err(invalid("nested or duplicate signature field name"));
            }
        }
        let mut signature = PdfDict::default();
        for (key, value) in [
            ("Type", name("Sig")),
            ("Filter", name("Adobe.PPKLite")),
            ("SubFilter", name("adbe.pkcs7.detached")),
        ] {
            signature.try_insert(key.into(), value)?;
        }
        let mut range = PdfArray::default();
        for _ in 0..4 {
            range.try_add_obj(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                b"00000000000000000000",
            )?))?;
        }
        signature.try_insert("ByteRange".into(), PdfObject::PdfArray(range))?;
        signature.try_insert(
            "Contents".into(),
            PdfObject::PdfString(PdfString::new(
                vec![b'0'; options.reserved_cms_bytes * 2],
                false,
            )),
        )?;
        let sig_id = document.try_insert_object(PdfObject::PdfDict(signature))?;
        let mut field = PdfDict::default();
        for (key, value) in [
            ("Type", name("Annot")),
            ("Subtype", name("Widget")),
            ("FT", name("Sig")),
            (
                "T",
                PdfObject::PdfString(PdfString::from_text_with_limits(
                    &options.field_name,
                    &limits,
                )?),
            ),
            ("V", reference(sig_id)),
            ("P", reference(first.page_id)),
            ("F", number(132)?),
        ] {
            field.try_insert(key.into(), value)?;
        }
        let mut rect = PdfArray::default();
        for _ in 0..4 {
            rect.try_add_obj(number(0)?)?;
        }
        field.try_insert("Rect".into(), PdfObject::PdfArray(rect))?;
        let mut appearance = PdfDict::default();
        appearance.try_insert("Type".into(), name("XObject"))?;
        appearance.try_insert("Subtype".into(), name("Form"))?;
        let mut bbox = PdfArray::default();
        for value in [0, 0, 1, 1] {
            bbox.try_add_obj(number(value)?)?;
        }
        appearance.try_insert("BBox".into(), PdfObject::PdfArray(bbox))?;
        appearance.try_insert("Resources".into(), PdfObject::PdfDict(PdfDict::default()))?;
        let appearance_id = document.try_insert_object(PdfObject::PdfStream(
            crate::objects::pdf_stream::PdfStream::new(appearance, Vec::new()),
        ))?;
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), reference(appearance_id))?;
        field.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
        let field_id = document.try_insert_object(PdfObject::PdfDict(field))?;
        fields.try_add_obj(reference(field_id))?;
        form.try_insert("Fields".into(), PdfObject::PdfArray(fields))?;
        form.try_insert("SigFlags".into(), number(3)?)?;
        let form_id = document.try_insert_object(PdfObject::PdfDict(form))?;
        catalog.try_insert("AcroForm".into(), reference(form_id))?;
        catalog.try_insert("Version".into(), name("2.0"))?;
        document.update_object(root_id, PdfObject::PdfDict(catalog))?;
        let (mut page, _page_lease) =
            crate::objects::clone_dict_with_budget(&first.page_dict, budget, limits, 0)?;
        let (mut annots, _annots_lease) = match page.get("Annots") {
            Some(value) => resolve_array(&document, value, budget)?,
            None => (PdfArray::default(), budget.reserve(0)?),
        };
        annots.try_add_obj(reference(field_id))?;
        page.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
        document.update_object(first.page_id, PdfObject::PdfDict(page))?;
        let output = document.append_signature_with_budget(budget)?;
        let (mut bytes, lease) = output.into_reserved_parts();
        let locations = signature_locations(&bytes, sig_id, limits)?;
        let contents = locations.contents;
        if bytes[contents.start] != b'(' || bytes[contents.end - 1] != b')' {
            return Err(invalid("unexpected placeholder representation"));
        }
        bytes[contents.start] = b'<';
        bytes[contents.end - 1] = b'>';
        let byte_range = [
            0,
            contents.start as u64,
            contents.end as u64,
            (bytes.len() - contents.end) as u64,
        ];
        let replacement = format!(
            "[{:020} {:020} {:020} {:020}]",
            byte_range[0], byte_range[1], byte_range[2], byte_range[3]
        );
        let destination = &mut bytes[locations.byte_range];
        if destination.len() != replacement.len() {
            return Err(invalid("ByteRange placeholder width mismatch"));
        }
        destination.copy_from_slice(replacement.as_bytes());
        let mut digest = Sha256::new();
        digest.update(&bytes[..contents.start]);
        digest.update(&bytes[contents.end..]);
        Ok(PreparedSignature {
            bytes: BudgetedBytes::from_reserved_vec(bytes, lease)?,
            contents,
            byte_range,
            digest: digest.finalize().into(),
            stamp: self.stamp(),
            completed: Cell::new(false),
            append_proof: None,
            field_proof: None,
        })
    }

    pub fn complete_signature(
        &self,
        prepared: &PreparedSignature,
        cms: &[u8],
    ) -> PdfResult<Vec<u8>> {
        let output = self.complete_signature_with_budget(
            prepared,
            cms,
            &self.ctx().limits().operation_budget(),
        )?;
        Ok(output.into_reserved_parts().0)
    }

    pub fn complete_signature_with_budget(
        &self,
        prepared: &PreparedSignature,
        cms: &[u8],
        budget: &OperationBudget,
    ) -> PdfResult<BudgetedBytes> {
        if !self.is_stamp_current(&prepared.stamp) || prepared.completed.get() {
            return Err(invalid("stale, foreign or consumed preparation"));
        }
        if cms.is_empty() || cms.len() > 64 * 1024 || cms.len() > (prepared.contents.len() - 2) / 2
        {
            return Err(invalid("CMS exceeds reservation or 64 KiB limit"));
        }
        validate_cms(cms, &prepared.digest, budget, true)?;
        let output = BudgetedBytes::copy_from_slice(budget, prepared.bytes.as_slice())?;
        let (mut bytes, lease) = output.into_reserved_parts();
        const HEX: &[u8] = b"0123456789ABCDEF";
        for (index, byte) in cms.iter().copied().enumerate() {
            bytes[prepared.contents.start + 1 + index * 2] = HEX[(byte >> 4) as usize];
            bytes[prepared.contents.start + 2 + index * 2] = HEX[(byte & 15) as usize];
        }
        let output = BudgetedBytes::from_reserved_vec(bytes, lease)?;
        if let Some(proof) = &prepared.append_proof {
            proof.validate_output(self, output.as_slice(), true, budget)?;
        }
        if let Some(proof) = &prepared.field_proof {
            proof.validate_output(self, output.as_slice(), true, budget)?;
        }
        prepared.completed.set(true);
        Ok(output)
    }

    pub fn signature_reports(&self) -> PdfResult<Vec<SignatureReport>> {
        let budget = self.ctx().limits().operation_budget();
        self.signature_reports_with_budget(&budget)
    }

    pub fn signature_reports_with_budget(
        &self,
        budget: &OperationBudget,
    ) -> PdfResult<Vec<SignatureReport>> {
        history::reports(self, budget, false)
    }

    /// Offline CMS verification of immutable source revisions, not trust,
    /// revocation, timestamps or authorization of subsequent modifications.
    pub fn verify_signatures(&self) -> PdfResult<Vec<SignatureReport>> {
        self.verify_signatures_with_budget(&self.ctx().limits().operation_budget())
    }

    pub fn verify_signatures_with_budget(
        &self,
        budget: &OperationBudget,
    ) -> PdfResult<Vec<SignatureReport>> {
        history::reports(self, budget, true)
    }
}

struct SignatureLocations {
    contents: Range<usize>,
    byte_range: Range<usize>,
}

// Parse the actual indirect signature object's dictionary. A whole-file marker
// search could bind ByteRange to attacker-controlled duplicate Contents bytes.
fn signature_locations(
    bytes: &[u8],
    id: ObjectId,
    limits: Limits,
) -> PdfResult<SignatureLocations> {
    signature_locations_with_budget(bytes, id, limits, &limits.operation_budget())
}

fn signature_locations_with_budget(
    bytes: &[u8],
    id: ObjectId,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<SignatureLocations> {
    let mut parser = SyntaxParser::with_options(
        StreamReader::try_new(Cursor::new(bytes))?,
        crate::parser::options::ParseOptions {
            mode: crate::parser::syntax::ParseMode::Strict,
            limits,
        },
    );
    parser.read_header()?;
    let (xref, _) = crate::parser::parser::parse_xref_with_offset(&mut parser)?;
    let info = xref
        .lookup_id(id)
        .filter(|i| matches!(i.state(), ObjectState::Normal))
        .ok_or_else(|| invalid("signature must be an uncompressed indirect object"))?;
    parser.set_pos(info.offset())?;
    if parser.get_next_word()?.as_u32_checked()? != id.obj_num()
        || parser.get_next_word()?.as_u32_checked()? != id.gen_num()
        || !parser.get_next_word()?.is_equal(b"obj")
        || !parser.get_next_word()?.is_equal(b"<<")
    {
        return Err(invalid("signature object header"));
    }
    let mut contents = None;
    let mut byte_range = None;
    for _ in 0..limits.max_objects {
        if parser.peek_word()?.is_equal(b">>") {
            parser.get_next_word()?;
            break;
        }
        let key = parser
            .get_object()?
            .into_name()
            .ok_or_else(|| invalid("signature dictionary key"))?;
        let start = skip_space(bytes, parser.position() as usize)?;
        let _value = parser.get_content_object_with_budget(budget)?;
        let end = parser.position() as usize;
        let slot = match key.name() {
            "Contents" => Some(&mut contents),
            "ByteRange" => Some(&mut byte_range),
            _ => None,
        };
        if let Some(slot) = slot
            && slot.replace(start..end).is_some()
        {
            return Err(invalid("duplicate signature key"));
        }
    }
    Ok(SignatureLocations {
        contents: contents.ok_or_else(|| invalid("missing Contents"))?,
        byte_range: byte_range.ok_or_else(|| invalid("missing ByteRange"))?,
    })
}
fn skip_space(bytes: &[u8], mut pos: usize) -> PdfResult<usize> {
    loop {
        while bytes
            .get(pos)
            .is_some_and(|b| matches!(b, 0 | 9 | 10 | 12 | 13 | 32))
        {
            pos += 1;
        }
        if bytes.get(pos) != Some(&b'%') {
            return if pos < bytes.len() {
                Ok(pos)
            } else {
                Err(invalid("unexpected EOF"))
            };
        }
        while bytes.get(pos).is_some_and(|b| *b != b'\r' && *b != b'\n') {
            pos += 1;
        }
    }
}

fn valid_algorithm_parameters(parameters: &Option<der::Any>) -> bool {
    use der::Tagged;
    parameters
        .as_ref()
        .is_none_or(|value| value.tag() == der::Tag::Null && value.value().is_empty())
}

fn validate_cms(
    bytes: &[u8],
    digest: &[u8; 32],
    budget: &OperationBudget,
    extended: bool,
) -> PdfResult<()> {
    use cms::{
        cert::CertificateChoices,
        content_info::ContentInfo,
        signed_data::{SignedData, SignerIdentifier},
    };
    use der::{
        Decode, Encode,
        asn1::{ObjectIdentifier, OctetString},
    };
    let _allocation = budget.reserve(
        bytes
            .len()
            .checked_mul(32)
            .and_then(|v| v.checked_add(256 * 1024))
            .ok_or_else(|| invalid("CMS allocation overflow"))?,
    )?;
    let oid = |value: &str| ObjectIdentifier::new(value).unwrap();
    let data_oid = oid("1.2.840.113549.1.7.1");
    let sha256 = oid("2.16.840.1.101.3.4.2.1");
    let rsa = oid("1.2.840.113549.1.1.1");
    let content = ContentInfo::from_der(bytes).map_err(|_| malformed("malformed DER CMS"))?;
    if content.content_type != oid("1.2.840.113549.1.7.2") {
        return Err(invalid("CMS SignedData required"));
    }
    let signed: SignedData = content
        .content
        .decode_as()
        .map_err(|_| malformed("malformed SignedData"))?;
    if signed.encap_content_info.econtent_type != data_oid
        || signed.encap_content_info.econtent.is_some()
        || signed.signer_infos.0.len() != 1
        || signed.digest_algorithms.len() != 1
        || signed.digest_algorithms.get(0).unwrap().oid != sha256
        || !valid_algorithm_parameters(&signed.digest_algorithms.get(0).unwrap().parameters)
    {
        return Err(invalid("single detached SHA-256 signer required"));
    }
    let signer = signed.signer_infos.0.get(0).unwrap();
    if signer.digest_alg.oid != sha256 || !valid_algorithm_parameters(&signer.digest_alg.parameters)
    {
        return Err(invalid("unsupported signature algorithm"));
    }
    let scheme = algorithms::scheme(signer, extended)?;
    let attrs = signer
        .signed_attrs
        .as_ref()
        .filter(|a| a.len() <= 32)
        .ok_or_else(|| invalid("bounded signed attributes required"))?;
    let mut found_digest = false;
    let mut found_type = false;
    for attr in attrs.iter() {
        if attr.oid == oid("1.2.840.113549.1.9.4") {
            if found_digest || attr.values.len() != 1 {
                return Err(malformed("duplicate messageDigest"));
            }
            let value: OctetString = attr
                .values
                .get(0)
                .unwrap()
                .decode_as()
                .map_err(|_| malformed("messageDigest encoding"))?;
            if value.as_bytes() != digest {
                return Err(PdfError::SignatureMismatch("CMS messageDigest".into()));
            }
            found_digest = true;
        } else if attr.oid == oid("1.2.840.113549.1.9.3") {
            if found_type
                || attr.values.len() != 1
                || attr
                    .values
                    .get(0)
                    .unwrap()
                    .decode_as::<ObjectIdentifier>()
                    .map_err(|_| malformed("contentType encoding"))?
                    != data_oid
            {
                return Err(malformed("CMS contentType"));
            }
            found_type = true;
        }
    }
    if !found_digest || !found_type {
        return Err(malformed("required signed attributes absent"));
    }
    let SignerIdentifier::IssuerAndSerialNumber(sid) = &signer.sid else {
        return Err(invalid("issuer/serial signer identifier required"));
    };
    let certs = signed
        .certificates
        .as_ref()
        .filter(|c| c.0.len() <= 16)
        .ok_or_else(|| invalid("bounded embedded certificates required"))?;
    let mut certificate = None;
    for choice in certs.0.iter() {
        if let CertificateChoices::Certificate(cert) = choice
            && cert.tbs_certificate.issuer == sid.issuer
            && cert.tbs_certificate.serial_number == sid.serial_number
            && certificate.replace(cert).is_some()
        {
            return Err(malformed("ambiguous signing certificate"));
        }
    }
    let key = &certificate
        .ok_or_else(|| malformed("signing certificate missing"))?
        .tbs_certificate
        .subject_public_key_info;
    match scheme {
        algorithms::Scheme::Rsa | algorithms::Scheme::Pss => {
            if key.algorithm.oid != rsa || !valid_algorithm_parameters(&key.algorithm.parameters) {
                return Err(invalid("unrestricted RSA certificate required"));
            }
        }
        algorithms::Scheme::Ec => {
            if key.algorithm.oid != oid("1.2.840.10045.2.1")
                || key
                    .algorithm
                    .parameters
                    .as_ref()
                    .and_then(|p| p.decode_as::<ObjectIdentifier>().ok())
                    != Some(oid("1.2.840.10045.3.1.7"))
            {
                return Err(invalid("named P-256 certificate required"));
            }
        }
    }
    let key_bytes = key
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| malformed("invalid RSA public key bits"))?;
    if matches!(scheme, algorithms::Scheme::Rsa | algorithms::Scheme::Pss) {
        algorithms::rsa_key_profile(key_bytes)?;
    }
    let attributes = attrs
        .to_der()
        .map_err(|_| malformed("signed attribute encoding"))?;
    let algorithm: &dyn ring::signature::VerificationAlgorithm = match scheme {
        algorithms::Scheme::Rsa => &ring::signature::RSA_PKCS1_2048_8192_SHA256,
        algorithms::Scheme::Pss => &ring::signature::RSA_PSS_2048_8192_SHA256,
        algorithms::Scheme::Ec => &ring::signature::ECDSA_P256_SHA256_ASN1,
    };
    ring::signature::UnparsedPublicKey::new(algorithm, key_bytes)
        .verify(&attributes, signer.signature.as_bytes())
        .map_err(|_| PdfError::SignatureMismatch("CMS cryptographic signature".into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn tiny_prepare_budget_does_not_publish_a_large_source_cache() {
        use crate::{
            budget::OperationBudget,
            document::Document,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::syntax::ParseMode,
        };
        let fixture = Document::new().unwrap();
        fixture.add_page(0).unwrap();
        let payload = fixture
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                vec![b'x'; 65536],
            )))
            .unwrap();
        let document =
            Document::from_bytes_with_mode(fixture.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        assert!(!document.ctx().source_object_is_cached(payload));
        let stamp = document.stamp();
        let small = OperationBudget::new(8192);
        assert!(
            document
                .prepare_signature_with_budget(Default::default(), &small)
                .is_err()
        );
        assert_eq!(small.live_bytes(), 0);
        assert!(!document.ctx().source_object_is_cached(payload));
        assert!(document.is_stamp_current(&stamp));
        let pool = OperationBudget::new(2 * 1024 * 1024);
        let prepared = document
            .prepare_signature_with_budget(Default::default(), &pool)
            .unwrap();
        assert!(document.ctx().source_object_is_cached(payload));
        drop(prepared);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn supported_algorithm_parameters_are_absent_or_empty_null_only() {
        use der::{Any, Decode};
        assert!(super::valid_algorithm_parameters(&None));
        assert!(super::valid_algorithm_parameters(&Some(
            Any::from_der(b"\x05\x00").unwrap()
        )));
        assert!(!super::valid_algorithm_parameters(&Some(
            Any::from_der(b"\x04\x00").unwrap()
        )));
        if let Ok(malformed_null) = Any::from_der(b"\x05\x01\x00") {
            assert!(!super::valid_algorithm_parameters(&Some(malformed_null)));
        }
    }
}
