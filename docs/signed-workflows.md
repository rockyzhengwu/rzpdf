# Controlled signed document workflows

Use the existing-field workflow to sign hierarchical fields, create visible
Unicode appearances, or apply a bounded form/comment update without rewriting
signed bytes. These APIs use immutable plans; they do not unlock ordinary
editing, full saves or ordinary incremental saves of signature-bearing input.

## Existing signature fields

Create all empty fields on an unsigned draft before signing. Select PDF 2.0
up front: preparation does not upgrade a signed or field-bearing input.

```rust,no_run
use pdf::{
    document::{Document, acroform::{FormWidgetSpec, FormTextStyle}, save_options::PdfVersion},
    parser::syntax::ParseMode,
    signatures::{SignatureFieldOptions, SignatureAppearance},
};
let draft = Document::new_with_version(PdfVersion::V2_0)?;
draft.add_page(0)?;
draft.add_signature_field("Approvals.Reviewer", &[
    FormWidgetSpec::new(0, [20., 180., 300., 230.]),
])?;
let doc = Document::from_bytes_with_mode(draft.save_to_bytes()?, ParseMode::Strict)?;
let prepared = doc.prepare_field_signature(SignatureFieldOptions {
    field_name: "Approvals.Reviewer".into(),
    reserved_cms_bytes: 16 * 1024,
    appearance: Some(SignatureAppearance {
        text: "Reviewed".into(),
        style: FormTextStyle::default(),
    }),
})?;
// Send the exact concatenation of prepared.signed_ranges() to an external
// detached CMS backend. Never perform raw RSA over sha256_digest().
let cms = std::fs::read("external-response.der")?;
let output = doc.complete_signature(&prepared, &cms)?;
// Store output using the host's atomic-file/publishing policy.
# Ok::<(), Box<dyn std::error::Error>>(())
```

`add_signature_field()` reuses AcroForm path creation and page/Widget ownership.
It supports multiple Widgets and zero-area invisible Widgets. The existing-field
preparation selects one uniquely named, empty terminal signature field; it does
not rename it, move Widgets, create fields on signed input or overwrite a value.
FT and flags may inherit through the field tree; a filled inherited signature
value, read-only field or unsupported Widget relationship is refused.
Annotation flag Locked (128) preserves geometry but does not prevent filling;
Widget ReadOnly (64) is ignored in favour of field Ff. LockedContents (512),
field read-only and unknown flag bits remain denied by this workflow.

All existing signatures must verify and remain unchanged. A previous unsigned
tail is accepted only if every actual revision passes the supported modification
audit. Reopen completed bytes strictly before another signature or update.

## Visible appearances

`SignatureAppearance` uses the existing form text encoder and layout. Supply
explicit LF-separated lines, a font size and alignment; text must fit every
Widget. There is no automatic wrapping, shaping, fallback or truncation. AP
streams own their resources and use a local positive BBox and identity Matrix;
page resources and shared fonts are not changed.

Default style uses bundled Helvetica. For Unicode, embed a suitable horizontal
font before signing and supply its handle from the **reopened source document**.
A handle from the original draft is foreign; retain its object ID and obtain a
new handle with `doc.object_handle(id)`. Fonts must already exist in the source
and be embedded or bundled. The core does not discover system fonts, embed new
font programs or modify source fonts on signed input. The default style may add
an AP-local Base14 font dictionary.

With `appearance: None`, existing AP streams, BBox, Matrix and resources are
preserved exactly. No appearance text, name or self-signed certificate establishes
a trusted identity. Visual content remains caller-specified artwork.

## Signed form and comment updates

Find a field handle with `form_field_tree()`, then call
`prepare_signed_form_fill(&handle, FormValue, style)` and
`complete_signed_update(&prepared)`. Supported values are `FormValue::Text` with
an explicit `FormTextStyle`, and `FormValue::Checkbox` with no style. Text updates
regenerate all Widget appearances. Checkbox updates reuse existing, unambiguous
on/off appearances and keep V/AS consistent. MaxLen and read-only restrictions
apply; password, file-select, comb, rich text, radio and choice updates are not
part of this signed-update profile.

The explicit style controls the generated AP only; it does not rewrite the
field's DA/Q or the AcroForm DR. Reopen and provide a style again for a later
controlled text update.

`prepare_signed_square(page_index, rect, contents)` prepares one new red outline
Square annotation with a self-contained appearance. It is not a generic annotation
editor: deletion, replies, popups, other annotation types and updates to existing
annotations remain unsupported. Complete it with `complete_signed_update()`.

Preparation never changes the source document or its overlay. Plans bind to
source identity/hash and stamp; source history, constraints, actual object
changes and all old signatures are checked before returning a preparation and
again at completion. Errors are retryable and release temporary reservations.
Foreign, rollback/panic-stale or successfully completed tokens cannot be reused.
A denied high-level edit can roll back a transaction and invalidate an earlier
token; prepare again on the unchanged source. Keep completed bytes if host
storage fails, rather than replaying a consumed token.

## Modification constraints

`signature_constraints()` reads declarations from each signed revision, binding
DocMDP to the unique first certification signature and FieldMDP to the original
field names and object identities. It does not itself establish that a signature
or certificate is trustworthy. DocMDP P must be an integer 1/2/3; omitted P means
2. Transform version is the name `/1.2`. FieldMDP supports All, Include and Exclude,
including descendants of named groups. Multiple signatures' restrictions intersect.
Unknown methods, versions, permissions, field sets or conflicting Lock/FieldMDP
declarations are not treated as permission to write. Signing a new Lock/SV-seeded
field and the optional Lock P profile remain unsupported.

| DocMDP | Existing text and checkbox fill | Existing empty-field signature | New Square annotation |
|---|---|---|---|
| P=1 | Denied | Denied | Denied |
| P=2 or omitted P | Subject to field locks | Subject to field locks | Denied |
| P=3 | Subject to field locks | Subject to field locks | Allowed in the bounded profile |

`signature_modifications()` compares actual consecutive source revisions. Each
report distinguishes `Allowed`, `Forbidden` and `Unsupported`, with modification
kinds and a reason where needed. `Appearance` identifies a value-preserving AP
update; field locks still apply. Cryptographic results still come from
`verify_signatures()`; `covers_current_document == false` can coexist with a valid
old signature and an authorized later fill. An empty modification list is not a
signature-validity verdict.

The classifier checks field ownership/names/geometry, object roles and reference
owners, trailer roots, fresh unshared APs, font reuse and page Annots projections.
It rejects unclassified objects, page/content/resource changes, shared-font edits,
structure/OCG changes, field renaming/removal and attempts to delete permissions
or signatures. Known V11-07 flat invisible approvals are recognized only without
MDP/field-lock declarations. No general new-field or page editing is authorized.

## Scope and limits

Controlled writes require strict immutable owned bytes, unencrypted PDF 2.0,
an indirect AcroForm, at most 256 field nodes and 16 signatures. Parser limits
also bound history depth, cumulative object/annotation visits, field names and
text. Hybrid history, compressed signature dictionaries, XFA, additional actions,
seed values, linearization, unknown/shared field owners and unsupported tagged or
optional-content Widget relationships are refused. Unmodified navigation,
structure and layers remain untouched; this does not establish PDF/UA compliance.

The `_with_budget()` variants share tracked operation allocations, AP work and
retained byte/proof leases. Conservative envelopes cover legacy graph builders
and declaration tables; not all parser/native allocations are charged. Ordinary
Vec reports do not retain a budget after returning. This is not a process-RSS
limit. Tiny or insufficient budgets fail without editing the source.

All operations preserve the original byte prefix. Old values and artwork remain
recoverable, so form filling is not redaction or secure erasure. Encrypted signing,
certificate trust/expiry/revocation, TSA, PAdES/LTV, private-key storage and arbitrary
signed-document rewriting remain outside this workflow.

## Validation

`docs/check-v011-workflows.py` uses synthetic local keys and existing OpenSSL,
pyHanko, pikepdf, qpdf, MuPDF and Skia tools. Explicit-P form/sign workflows use
pyHanko's diff policy. Its P=3 annotation policy is not implemented, and it requires
P even where the standard provides a default: those cases use independent exact
object projections and direct parameter checks, with pyHanko or OpenSSL verifying
CMS mathematics separately. Render-only reference PDFs paint independent
page-level artwork rather than copying the core AP streams; both renderers compare
actual and reference pixels. Final evidence is in the
[v0.11 acceptance record](releases/v0.11-acceptance.md).

The independent producer's original Courier appearance also has a cross-renderer
text-bounds check and visual review. This catches missing-width fallback errors
that within-renderer reference pairs could share. Explicit zero widths remain
zero; codes outside a simple font's Widths range use FontDescriptor MissingWidth
(default zero).

Rules follow the applicable signature clauses of
[ISO 32000-1](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)
and [PDF 2.0 interactive-feature corrections](https://pdf-issues.pdfa.org/32000-2-2020/clause12.html).
The supported subset is narrower than all operations the standard permits.
