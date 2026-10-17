# Offline signature verification

Use `Document::verify_signatures()` to check detached CMS signatures against
their original source revisions. A `Valid` result means that the PDF byte digest
and cryptographic signature match. It does not establish certificate trust,
expiry, revocation, timestamps, PAdES conformance, or permission for later changes.
The API performs no network requests and accepts no private keys.

```rust,no_run
use pdf::{document::Document, parser::syntax::ParseMode};
let doc = Document::from_bytes_with_mode(std::fs::read("signed.pdf")?, ParseMode::Strict)?;
for report in doc.verify_signatures()? {
    println!("revision={} verification={:?} current={}",
        report.signed_revision, report.verification, report.covers_current_document);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Source revisions and fields

The input must be strict immutable owned bytes. The implementation follows the
actual xref/Prev chain and reads each historical object graph independently;
mutable edits in the current overlay do not participate. It binds the ByteRange
hole to that signature dictionary's exact Contents token, and the covered end
to the EOF of the revision where the signature first appears. Compressed
signature dictionaries, hybrid history, encrypted history, inconsistent ranges,
invalid EOF boundaries, and non-backward Prev chains are refused.
Other active objects may neither start inside nor extend across the excluded
Contents span; otherwise unsigned CMS bytes could also act as document content.

`signed_revision` is a zero-based index, oldest first. `signed_revision_xref`
identifies its xref address; `signed_revision_bytes` is the covered prefix length.
`field` records the indirect field identity and qualified name from that
historical revision, including inherited field types and hierarchical names.
An orphan signature has no field association. `source_object_state` reports
whether the signature object is unchanged, replaced, or removed in the latest
source xref. Replacement or removal cannot erase the historical report.

`covers_current_document` describes exact source-byte coverage, not permission
for later revisions. An old signature can remain cryptographically valid while
no longer covering the complete input or while its dictionary has been removed.
Current and retired object versions, including object-stream members, remain
subject to conservative signed-source write protection.

## Verification results and supported algorithms

`signature_reports()` performs structural inspection and keeps verification
`NotPerformed`. `verify_signatures()` adds offline CMS checks, reporting `Valid`,
`Invalid` for digest/signature mismatch, `Malformed` for invalid CMS, or
`Unsupported` for profiles outside the supported subset. Invalid source history
or structural coverage returns an error; it is not reported as a valid signature.
Resource and I/O failures also propagate as errors.

Supported SubFilter is `adbe.pkcs7.detached`, with one detached DER CMS SignerInfo,
SHA-256, issuer/serial certificate selection, embedded certificates and required
contentType/messageDigest signed attributes. CMS is limited to 64 KiB; only zero
padding may follow its DER object. Certificate trust is a separate host concern.

| Algorithm | Profile |
|---|---|
| RSA PKCS#1 v1.5 | SHA-256; 2048–8192-bit RSA key; existing PDF version bounds |
| RSA-PSS | PDF 2.0; SHA-256/MGF1-SHA-256; salt32; omitted DEFAULT trailer1; unrestricted RSA SPKI only |
| ECDSA | PDF 2.0; SHA-256; named P-256 curve; DER signature; absent signature algorithm parameters |

The same bounded algorithms are accepted when completing a new prepared
signature. `prepare_signature()` creates the first invisible approval on
unsigned input. `prepare_additional_signature()` appends another invisible
approval within the [bounded signing workflow](advanced-saving.md#additional-approval-signatures).
Neither API enables general signed-document editing or visible appearances.
Use `prepare_field_signature()` for existing hierarchical fields and explicit
visible appearances; see [controlled workflows](signed-workflows.md).

## Constraints and subsequent changes

`signature_constraints()` reads historical DocMDP/FieldMDP declarations, not
certificate trust. `signature_modifications()` combines cryptographic checks,
signed constraints and actual revision-object comparisons. Reports distinguish
known fills, value-preserving appearances, signatures and supported annotations,
returning Allowed, Forbidden or Unsupported. An error or Unsupported result must
not be interpreted as permission. An empty list means there is no later revision
to classify, not that the signature or certificate is valid.

The classifier is intentionally bounded: it recognizes existing text/checkbox
fills, existing-field signatures, unconstrained V11-07 invisible approvals and
new Square annotations, preserving unrelated objects and relationships. Unknown
transforms, owner aliases, fields renamed/deleted, shared resources or page/body/
structure/OCG changes block the dedicated writes. General editing stays denied.
Detailed permission intersections and verifier limitations are in the
[workflow guide](signed-workflows.md#modification-constraints).

## Limits and independent validation

`signature_reports_with_budget()` and `verify_signatures_with_budget()` share the
caller pool through object/CMS parsing and retained field-name/ownership tables.
An envelope estimates legacy
history/field overhead; legacy parser allocations are not all automatically
charged to that pool. Parser limits separately bound revision depth, cumulative object
visits, field depth/names, certificates, attributes and signature count (16).
This is tracked operation accounting, not an allocator-wide or process-RSS guarantee. A failure
does not mutate the document or consume a signing preparation.

`docs/check-v011-signatures.py NEW_DIRECTORY` uses synthetic local keys and
OpenSSL/pyHanko producers. It checks cryptography and history, not modification
authorization; independent verification disables diff-policy evaluation.
Build `v011_sign_verify` first and use the existing environment documented in
the script. No production credentials, dependency installation or network trust
fetching is involved. Current implementation and evidence are recorded in the
[v0.11 acceptance record](releases/v0.11-acceptance.md).
