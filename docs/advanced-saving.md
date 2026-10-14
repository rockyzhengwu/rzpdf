# Advanced saving (v0.5 bounded APIs)

See the [support matrix](releases/v0.5-support.md) and
[acceptance record](releases/v0.5-acceptance.md) for actual verified scope/status.
These are separate workflows; encrypted incremental saves, encrypted signatures
and editing already signed input are deliberately unsupported.

## Append changes without rewriting the source

```rust,no_run
use pdf::{document::Document, parser::syntax::ParseMode};
let doc = Document::from_bytes_with_mode(std::fs::read("input.pdf")?, ParseMode::Strict)?;
doc.rotate_page(0, 90)?;
doc.save_incremental_to_file("updated.pdf")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Source must be strict immutable owned bytes, unsigned, unencrypted and not
linearized. Output preserves all original bytes, including removed private
content. This is not redaction. Saving never rebases the document or clears
edits; reopen the output to append the next revision. File/reader/new-document
inputs must first full-save and reopen as owned bytes.

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

`v05_sign` demonstrates an OpenSSL backend. Only a single invisible approval
signature on new/strict owned-byte unsigned, unencrypted input is supported.
Up to 256 flat v0.4 text/checkbox fields are retained; XFA, complex fields, NeedAppearances,
duplicate names and existing signatures are rejected. Owned source older than
PDF 1.4 must first full-save at a newer version and reopen. New preparation uses
a PDF 2.0 snapshot; owned source is preserved and Catalog version upgraded.

Completion accepts one detached DER CMS signer using SHA-256, issuer/serial
certificate identification, required signed attributes and RSA PKCS#1 v1.5
2048..8192 bits. It checks messageDigest and the RSA signature. It does **not**
check certificate trust, expiry, revocation, TSA/LTV, PAdES or DocMDP/FieldMDP.
RSA-PSS/ECDSA are not supported. CMS cap is 64 KiB; default reservation is 16 KiB
(configurable 1 KiB..1 MiB subject to parser limits). Too-small space is an error,
not a silent resize after signing.

Preparation is immutable and bound to document identity/revision. Edits and
mutating rollbacks invalidate it. Failed completion is retryable; successful
completion is single-use. Only Contents is filled; signed bytes never change.
`signature_reports()` describes original immutable source coverage and returns
NotPerformed, never an unverified “valid/trusted” verdict. Existing
signature-bearing sources are rejected at low-level mutation and full-save
boundaries, including attempts to remove signature objects first.

Budget-aware prepare/complete APIs retain output reservations for the lifetime
of their returned owner. Shared OperationBudget pools aggregate retained bytes;
these are tracked operation allocations, not process-RSS limits. Serialization
failures leave source/state unchanged. Caller-owned Write failures can leave
partial output; retry using a fresh target. Completion returns bytes so the host
can use its own atomic storage/publishing policy.

## Upgrade notes

`FilterContext` has a new `crypt_already_processed` field. Prefer struct update
syntax with `..Default::default()` when constructing it. The old unimplemented
`parser::security::SecurityHandler` name re-exports the real handler, but the
insecure/stub Default constructor is removed: authenticate via document password
constructors. Default save of authenticated documents now preserves security;
signed-source edits/rewrites that previously invalidated signatures now fail.
