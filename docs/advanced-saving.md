# Advanced saving

For file-level diagnostics and explicitly read-only damaged-index input, see [file diagnostics and recovery](file-recovery.md). Recovery is a separate capability, not an additional save layout or an automatic repair step. [API/platform compatibility](releases/v0.12-platform.md) records defaults, migration and output failure contracts.

See the [v0.5 baseline](releases/v0.5-support.md) for existing signing and
protection limits, and the [v0.11 support matrix](releases/v0.11-support.md) for
authenticated R6 incremental saving, [offline historical signature verification](signature-verification.md)
and bounded additional approvals. Encrypted signatures and general editing of
already signed input remain unsupported. Dedicated [controlled workflows](signed-workflows.md)
can sign existing fields or produce bounded updates without removing signed-source guards.

## Explicit full-save layouts

For private keys, trailer roots, AF/XMP, sharing and source-container handling,
see the [object and relationship preservation matrix](object-preservation.md).
Saving and page transfer have different ownership guarantees.

Existing save methods and `SaveOptions` literals still produce classic xref
tables. To select an uncompressed cross-reference stream explicitly:

```rust,no_run
use pdf::document::{Document, save_options::{SaveLayout, SaveOptions}};
let doc = Document::new()?; // PDF 1.7 by default
doc.add_page(0)?;
let bytes = doc.save_to_bytes_with_layout(SaveOptions::default(), SaveLayout::XrefStream)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

These full-rewrite profiles require unsigned input. Unencrypted output needs
effective PDF 1.5 or later; choose an explicit `SaveOptions.version` upgrade
for a lower version. Existing R6 protection is preserved automatically and
requires PDF 2.0 or the supported PDF 1.7 ADBE extension level 8 declaration.
Adding or replacing R6 requires PDF 2.0, as with existing security saves.
Source-signature checks remain in force. Recognized linearized input can be
fully rewritten as an ordinary, non-linearized PDF in all three layouts.
The source parameter dictionary and its primary/overflow hint streams are
omitted from output; their IDs become free entries. The in-memory document
and original bytes are unchanged. This is not linearized output or a general
garbage collector: unrelated objects retain their identities.

Removal requires a source-first parameter dictionary with known fields and
resolvable hint streams. Modified structures, unknown fields, malformed hints,
or references to these structures from retained objects/trailer roots cause
explicit refusal. Unrecognized `/Linearized` declarations also refuse in the
classic layout rather than retaining stale offsets. Incremental saving of
linearized input remains unsupported. No hint-table migration, Range loading
or linearized generation is provided. Full saving does not remove declarations
from signed source bytes: signed sources remain protected. A linearized base's
forward-linked xref pair also remains outside signature-history verification;
that path reports an explicit unsupported-feature error, not a corruption verdict.
See the [native/browser validation record](testing.md#linearized-input-ordinary-full-save-regression).
Stream-style incremental output has a separate explicit option described below.

For non-linearized input, `SaveLayout::Classic` follows the existing serializer
and preserves its default byte output. The new `XrefStream` profile emits independent objects and a fresh
output-only xref object, with `/W [1 8 4]` and `/Index [0 Size]`; it does not
mutate the document, reserve a persistent object ID or rebase its source.
Holes remain free entries within the configured Size limit. The xref payload
is uncompressed even when ordinary streams use `FlateUnencoded`; this is not
a minimum-file-size guarantee or secure erasure.

`save_with_layout_and_budget()` returns BudgetedBytes retaining its operation
reservation. Matching writer/file methods are available; serialization finishes
before host writes, while an I/O error may leave partial caller-owned output.
The file method uses the existing atomic-replacement contract, not create-new
semantics. The `v012_layout` example intentionally uses create-new output.

See the [frozen boundaries](releases/v0.12-boundaries.md) and
[milestone evidence](releases/v0.12-acceptance.md) for scope and validation.

### Object-stream packing

Select `SaveLayout::XrefStreamWithObjectStreams` to rebuild object containers
while preserving business object IDs and values. The same four layout save
methods apply. Groups contain at most 100 members and 1 MiB of decoded index
and member data, intersected with stream and operation limits. A member that
cannot fit alone remains an independent object; this is not an output error.

Streams, nonzero generations, Catalog, Encrypt, indirect stream-length
dependencies, reference-only objects and signature/placeholder dictionaries
stay independent. With `StreamCompression::Preserve`, newly generated ObjStm
payloads are unencoded. With `FlateUnencoded`, the packer uses Flate only when
it reduces the payload and fits the raw-stream limit. The xref stream remains
uncompressed in both cases; no compression ratio is promised.

Old XRef/ObjStm containers are replaced only under a conservative profile.
`Extends` collections, unknown container fields, or ordinary/trailer references
to a replaced container cause explicit refusal. Ordinary private dictionaries,
shared/cyclic references, metadata and attachments are not garbage-collected.
The independent-object `XrefStream` mode retains its previous behavior; its
selection does not silently opt into this container-replacement policy.
Neither mode promises secure erasure of historical/private content.

### Layout and protection together

The ordinary layout methods preserve authenticated R6 protection, including
when opened with a user password. Changing/removing existing protection needs
owner authentication, through explicit combined entry points:

```rust,no_run
use pdf::{document::{Document, save_options::{SaveLayout, SaveOptions}},
          security::{EncryptionOptions, SaveSecurity}};
let doc = Document::new()?;
doc.add_page(0)?;
let settings = EncryptionOptions::new("illustrative-user", "illustrative-owner")?;
let bytes = doc.save_to_bytes_with_layout_and_security(
    SaveOptions::default(), SaveLayout::XrefStreamWithObjectStreams,
    SaveSecurity::Set(&settings),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Corresponding methods are `save_with_layout_and_security_and_budget`,
`save_to_writer_with_layout_and_security` and
`save_to_file_with_layout_and_security`. `SaveSecurity::Remove` is explicit;
editing mutable Encrypt entries does not remove source protection. Use protected
host credential input in real applications; strings above are illustrative.

Members are serialized in plaintext, optionally compressed together, then the
containing ObjStm uses the handler's default stream filter exactly once. XRef
and active Encrypt stay unencrypted; Encrypt, Catalog, metadata and ordinary
streams never enter the packer. If default string and stream filters differ,
objects containing strings remain independent. Clear metadata and explicit
Crypt/Identity streams keep their existing exceptions. AES grouping reserves
space for IV and padding in the raw-stream limit; this remains a conservative
packing strategy, not a minimum-size guarantee.

New layouts reuse an existing indirect Encrypt identity, including generation;
direct/new Encrypt receives a collision-checked output-only ID. File IDs and
permission policy follow existing full-save security rules. Removal may retain
the old Encrypt dictionary as an ordinary object; neither full rewriting nor
protection changes promise removal of historical/private data. R2-R5, public-key
encryption and encrypted signatures remain unsupported.

## Append changes without rewriting the source

```rust,no_run
use pdf::{document::Document, parser::syntax::ParseMode};
let doc = Document::from_bytes_with_mode(std::fs::read("input.pdf")?, ParseMode::Strict)?;
doc.rotate_page(0, 90)?;
doc.save_incremental_to_file("updated.pdf")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Source must be strict immutable owned bytes, unsigned and not linearized.
Unencrypted inputs retain their original contract. Authenticated Standard R6
inputs may preserve their encryption when appended, as detailed below.
Output preserves all original bytes, including removed private
content. This is not redaction. Saving never rebases the document or clears
edits; reopen the output to append the next revision. File/reader/new-document
inputs must first full-save and reopen as owned bytes.

### Explicit incremental cross-reference streams

```rust,no_run
use pdf::{
    document::{Document, save_options::{IncrementalSaveLayout, IncrementalSaveOptions}},
    parser::syntax::ParseMode,
};
let doc = Document::from_bytes_with_mode(std::fs::read("input.pdf")?, ParseMode::Strict)?;
doc.rotate_page(0, 90)?;
let bytes = doc.save_incremental_to_bytes_with_layout(
    IncrementalSaveOptions::default(), IncrementalSaveLayout::XrefStream,
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The separate non-exhaustive `IncrementalSaveLayout` offers Classic (default)
and XrefStream; it does not change the fields of `IncrementalSaveOptions`.
Existing methods, signature preparation and controlled updates retain their
classic layout. Matching `save_incremental_with_layout_and_budget`,
`save_incremental_to_writer_with_layout` and `save_incremental_to_file_with_layout`
methods accept the same options and layout; the budgeted method additionally
accepts the shared operation budget and retains the output reservation.

An actual stream append requires effective PDF 1.5+. It does not silently
upgrade the version: use an explicit Catalog Version edit or full rewrite and
reopen. No-op/rolled-back saves still return the original bytes, even for a
lower version, without generating a new xref object or requesting randomness.
Unsigned table, stream and hybrid sources can be reopened for mixed-layout
revision chains. Prev points to the last physical section, not a hybrid
supplement; the new revision does not inherit XRefStm.

The tail uses plaintext, uncompressed `/W [1 8 4]` records and sparse `/Index`
runs for changed objects, known free entries, object zero and its own entry.
Untouched entries inherit through Prev. Modified compressed members become
ordinary generation-zero objects; no new ObjStm is generated. The fresh xref
ID is above the source Size and all source/current object numbers, ephemeral,
and cannot capture existing dangling references. Size, payload, descriptor,
operation and output limits remain enforced; a rejected save does not commit
or consume the document's allocator. Full serialization precedes host writes;
file output uses atomic replacement, while a host writer can fail partially.

### Authenticated R6 incremental saves

Open protected immutable bytes with `Document::from_bytes_with_password_and_options`
and strict options, make a permitted edit, then call the same incremental API.
Credentials should come from protected host input, not argv or logs. Both user
and owner may append permitted edits; modification/form permissions are unchanged.

Supported sources are PDF 2.0 or PDF 1.7 with ADBE extension level at least 8,
using Standard V=5/R=6 and the existing supported crypt filters. Direct and
indirect Encrypt dictionaries are preserved. Only changed output objects are
encrypted, including members updated from an original ObjStm; untouched source
bytes are not rewritten. Identity streams and clear metadata remain plaintext
exceptions. The first file ID is preserved; actual append generates a new second
ID. No-op/rolled-back changes return exact source bytes. IVs and revision IDs are
randomized, so nonempty saves need not be byte-identical; saving never rebases.

Low-level mutation/removal/replacement of Encrypt or source IDs is refused.
Use explicit owner-authorized full save to change protection. Legacy R2..5,
public-key encryption, encrypted signing and signed encrypted input remain
outside this subset. See the [v0.11 acceptance record](releases/v0.11-acceptance.md)
for the verified scope and current phase status.

## Password protection

```rust,no_run
use pdf::{document::Document, security::{EncryptionOptions, SaveSecurity}};
let doc = Document::new()?;
doc.add_page(0)?;
let settings = EncryptionOptions::new("example-user", "example-owner")?;
let protected = doc.save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))?;
let opened = Document::from_bytes_with_password(protected, "example-owner")?;
assert!(opened.security_report().is_some());
let still_protected = opened.save_to_bytes()?; // Preserve is the default
let plaintext = opened.save_to_bytes_with_security(Default::default(), SaveSecurity::Remove)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Examples above contain public illustrative strings only. Production passwords
should come from the host application's protected credential input, not argv or
logs. Existing protection can be removed/replaced only after owner authentication.
Changing mutable Encrypt entries never changes the default preservation policy.
R=6 passwords use UTF-8/SASLprep, with 127 normalized-byte maximum and explicit
rejection rather than truncation. Empty user passwords are allowed; new owner
passwords must be nonempty. No password cracking or legacy RC4/R=2..5 support.

Permission flags are advisory, not DRM. High-level annotations/assembly check
their flags plus modify; form filling requires fill_forms, with a scoped internal
exception to modify. Low-level object edits check modify but cannot classify
every semantic change. Printing/copying/accessibility flags are reported for the
host, not enforced over plaintext already returned. Owner access can edit/manage
protection. New PDF 2.0 output requires accessibility=true.

OS randomness supplies salts, IVs and file keys. Controlled secret buffers are
zeroized, but caller inputs, all compiler temporaries and plaintext object caches
are not guaranteed erased. AES-CBC protection does not authenticate PDF contents.

## External detached CMS signing

```rust,no_run
use pdf::{document::Document, parser::syntax::ParseMode, signatures::SignatureOptions};
let doc = Document::from_bytes_with_mode(std::fs::read("unsigned.pdf")?, ParseMode::Strict)?;
let prepared = doc.prepare_signature(SignatureOptions::default())?;
let (first, second) = prepared.signed_ranges();
// Give the concatenation first || second to an external detached CMS backend.
// SHA-256 CMS signed attributes are signed by that backend, NOT a raw RSA
// operation over prepared.sha256_digest(). Private keys never enter PDF core.
let cms = std::fs::read("external-result.der")?;
let output = doc.complete_signature(&prepared, &cms)?;
std::fs::write("signed.pdf", output)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`v05_sign` demonstrates an OpenSSL backend. `prepare_signature()` prepares the
first invisible approval on new/strict owned-byte unsigned, unencrypted input.
Signing accepts traditional xrefs and pure xref/object streams, but rejects
hybrid-reference history because of cross-reader ambiguity. Full-rewrite and
reopen such a document explicitly before signing; preparation never silently
normalizes the source. Removing a mutable XRefStm key cannot hide the history.
Up to 256 flat v0.4 text/checkbox fields are retained; XFA, complex fields, NeedAppearances,
duplicate names and existing signatures are rejected. Owned source older than
PDF 1.4 must first full-save at a newer version and reopen. New preparation uses
a PDF 2.0 snapshot; owned source is preserved and Catalog version upgraded.

Completion accepts one detached DER CMS signer using SHA-256, issuer/serial
certificate identification, required signed attributes and RSA PKCS#1 v1.5
2048..8192 bits, or the bounded PDF 2.0 RSA-PSS/P-256 ECDSA profiles described in
the [offline verification guide](signature-verification.md). It checks
messageDigest and the cryptographic signature. It does **not**
check certificate trust, expiry, revocation, TSA/LTV or PAdES. CMS verification
alone does not authorize modifications; the dedicated existing-field/update
workflows separately check their supported DocMDP/FieldMDP profile.
CMS cap is 64 KiB; default reservation is 16 KiB
(configurable 1 KiB..1 MiB subject to parser limits). Too-small space is an error,
not a silent resize after signing.

Preparation is immutable and bound to document identity/revision. Edits and
mutating rollbacks invalidate it. Failed completion is retryable; successful
completion is single-use. Only Contents is filled; signed bytes never change.
`signature_reports()` describes original immutable source coverage and returns
NotPerformed, never an unverified “valid/trusted” verdict.
`verify_signatures()` additionally checks historical CMS and reports cryptographic
results separately from trust and subsequent modification authorization. Existing
signature-bearing sources are rejected at low-level mutation and full-save
boundaries, including attempts to remove signature objects first.

Budget-aware prepare/complete APIs retain output reservations for the lifetime
of their returned owner. Shared OperationBudget pools aggregate retained bytes;
these are tracked operation allocations, not process-RSS limits. Serialization
failures leave source/state unchanged. Caller-owned Write failures can leave
partial output; retry using a fresh target. Completion returns bytes so the host
can use its own atomic storage/publishing policy.

## Additional approval signatures

`prepare_additional_signature()` and its `_with_budget()` variant prepare a new
invisible approval field on a strict immutable owned-byte, unencrypted PDF 2.0.
Pass the result to the same external detached CMS backend and
`complete_signature()` used above. Reopen completed bytes strictly before
preparing another signature. `v011_sign_verify append` is an offline test driver
for this workflow, not a private-key storage API.

Every existing signature must verify, remain unchanged and bind to a filled
flat signature Widget. The latest signature must cover the entire input;
unsigned tails, replaced/deleted/orphan signatures, hybrid history and pending
edits are refused. The document must have an indirect AcroForm and at most 256
flat signature/text/checkbox Widgets with unambiguous page ownership. The API
adds a uniquely named field, not a value to an existing empty signature field.
The total signature limit is 16. Input versions are not silently upgraded.

All historical DocMDP/FieldMDP, Perms, Reference, Lock, seed-value, additional
action, XFA and linearization declarations are conservatively refused. This
includes DocMDP permissions 2/3: support for interpreting and authorizing their
allowed operations belongs to the later modification-policy stage.

Preparation constructs a private immutable plan bound to the source hash and
document stamp. It does not edit the document or relax signed-source guards.
Only AcroForm Fields/SigFlags and the first page's Annots are replaced; three
fresh objects hold the signature, zero-area Widget and empty appearance. Other
old objects and the entire original byte prefix remain unchanged. Both
preparation and completion strictly reopen and recheck every old signature and
the new signature's field/coverage. The independent multi-sign gate also checks
the exact object projection with pyHanko's diff policy enabled.

Failed CMS completion remains retryable; successful completion consumes only
that token. Foreign documents and rollback/panic-stale tokens are rejected.
Even a denied high-level edit may roll back a transaction and invalidate its
previous token: prepare again on the unchanged signed source. Preparation and
output retain tracked shared-budget leases; legacy construction uses a
conservative envelope, not an allocator-wide or process-RSS guarantee. General
edits, full saves and ordinary incremental saves of signed input remain denied.

## Upgrade notes

For hierarchical/multi-Widget existing fields, visible Unicode appearances,
DocMDP/FieldMDP inspection and bounded signed form/comment updates, use the
[controlled signed-document guide](signed-workflows.md). The older
`prepare_signature()` and `prepare_additional_signature()` retain their
unsigned/flat-unconstrained input limits; they do not silently switch workflows.

`FilterContext` has a new `crypt_already_processed` field. Prefer struct update
syntax with `..Default::default()` when constructing it. The old unimplemented
`parser::security::SecurityHandler` name re-exports the real handler, but the
insecure/stub Default constructor is removed: authenticate via document password
constructors. Default save of authenticated documents now preserves security;
signed-source edits/rewrites that previously invalidated signatures now fail.
