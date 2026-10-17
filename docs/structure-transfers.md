# Structure aware page operations

V10-05 connects the supported logical structure graph to existing page copy,
import, selection, merge, deletion and reorder operations. Snapshot-based body
edits validate content ownership before and after publication. Callers still
choose document semantics: these operations do not infer structure, recompute
replacement text or certify Tagged PDF/PDF-UA conformance.

## Copy and import

```rust,ignore
use pdf::document::page_transfer::PageTransferOptions;

doc.copy_page_with_options(0, 2, &PageTransferOptions {
    name_prefix: "copy".into(),
    ..Default::default()
})?;
target.import_pages_from_with_options(&source, &[1, 0], 0,
    &PageTransferOptions::default())?;
let selected = source.extract_pages(&[1])?;
```

Supported structure is retained automatically, without a drop-tags fallback.
The writer projects content belonging to selected pages and retains its ancestor
elements. Page-bound metadata-only nodes are retained when their page is selected;
all-page imports also retain unbound empty nodes. A partial selection cannot
assign page-unbound element ActualText or split an element replacement spanning
selected and unselected pages; it rejects rather than guessing a substring.

Copied/imported branches append to the target root in source K order. The order
of selected page indices controls physical page insertion, not declared reading
order. Existing target branches keep their identities and order. MCR/OBJR Pg,
Form Stm, object pointers and child P links use the shared importer ID map.
Integer K items are normalized to explicit page-qualified MCRs before moving.

New pages, structural Forms and whole-object/annotation owners receive fresh
ParentTree keys respecting the target's NextKey. MCIDs remain unchanged inside
their page/Form domain; null slots are retained, and reverse owner references
are rebuilt. Numeric object IDs from different documents are never identities.

Same-document structure copies clone the selected resource graph instead of
mutating original streams or sharing structural Form ownership. This deliberately
costs more than an unstructured copy, which can share immutable content/resources.
One imported batch still shares one ID map, preserving sharing within that batch.
Source objects and unselected original pages remain unchanged.

If a Form's declared Pg is outside the selection, its declaration can move to a
unique selected page that actually invokes it. Zero candidates leaves the branch
outside the projection; multiple candidates reject. Partial selection of one
Form's MCIDs across incompatible declared pages can also reject at final ownership
validation. No Form instance or structure owner is selected heuristically.

Existing annotation and Widget OBJR associations migrate with annotation/field
ownership and ParentTree. AP bytes and local resource graphs are retained. The
page-specific field transfer guard permits validated StructParent ownership;
ordinary field edit guards remain conservative. The existing field/destination
name prefix policy is unchanged.

## Role maps attributes and namespaces

RoleMap, ClassMap, node A/C/R and namespace references are retained. Conflicting
root map names reject; disjoint names merge. Identical direct definitions can
share a name, but equal reference numbers do not prove cross-document equivalence.
Opaque or indirectly referenced conflicting definitions require explicit
reconciliation first. `name_prefix` does not silently rename roles or classes.

After merging, qualified role endpoints and class-use states must remain the
same for both existing target nodes and incoming nodes. A new map entry cannot
capture a previously unmapped role or undefined class in either graph. Version
changes that would alter the ordering model of multiple attribute attachments
also reject instead of silently changing their interpretation.

Same-document copies reuse immutable namespace definitions, avoiding same-URI
dictionary aliases. A distinct-source namespace whose URI already exists in the
target rejects; the caller must reconcile the definitions explicitly. The core
does not fetch namespace schemas or infer their semantics.

## Delete and reorder

Deletion removes page-owned K items, prunes branches emptied by that removal and
rebuilds the live reverse index. Surviving node identities, mixed K order and
ParentTreeNextKey are retained. Existing empty unbound nodes remain, except where
their replacement text cannot be assigned safely. A structural Form declaration
whose Pg is deleted can rebind to its unique remaining invocation page; multiple
remaining pages reject. Removing only part of an element ActualText also rejects.

Navigation retargeting is separate: `PageDeletionPolicy::Retarget` changes links,
outlines and destinations, not the logical text of the removed page. Annotation
and Widget detachment follows the existing page relationship transaction. Unknown
live references to the deleted page reject the whole operation. Detachment is not
secure erasure; historical objects, content bytes and replacement text can remain.

Reorder preserves page IDs, structure K order and ownership. Physical page order
therefore need not match declared reading order. Existing inherited resource/box
handling and page-bound labels remain active.

## Body edits

The existing snapshot-based text/path/image/operation edit commit checks supported
MCID/MCR/OBJR associations before preparation and after writing a fresh content
stream. An edit that removes a declared MCID, introduces an unowned one, removes a
referenced Form or changes declared ownership rejects within the transaction.
Ordinary graphics Forms inside page MCIDs can still use existing isolated nested
editing. Editing a Form that has its own structural MCIDs currently rejects when
the cloned Form/domain cannot be kept coherent; it is not silently detached.

ActualText is explicit metadata, not a derived copy of glyph text. Changing or
clearing glyphs preserves element/marked replacement strings. Update those
declarations explicitly if their meaning must change. Existing strict font and
content-source restrictions still apply; an ownership check does not expand the
supported interpreter model.

Raw object updates, raw content replacement/appending and resource registration
remain low-level construction tools, not graph-preserving edit APIs. They may
temporarily create inconsistent relationships; validate `structure_associations()`
before relying on such a document. The snapshot edit path refuses an already
incoherent source rather than repairing it implicitly.

## Safety and verification

Page, annotation, modification, signature and source-version policies remain in
force. Encrypted import sources and newer-version sources still reject. Unknown
structure/root/ParentTree extensions, IDTree/Ref relationships, appearance-stream
MCR/StmOwn and unsupported ownership domains reject before rewriting. There is no
option that bypasses these checks by dropping tags.

OCProperties configurations and OCG/OCMD or OC-bearing XObject import currently
reject; configuration migration remains outside the v0.10 supported subset.
Stage C adds [explicit visibility evaluation](optional-content.md), not an
implicit relaxation of these transfer guards. Copying
layer references without their configuration is not a supported fallback.

Existing decode/operation/depth/count limits apply to each structural scan;
imported graph payloads and reverse slots have additional cumulative limits.
The projection graph and temporary standard containers are not fully covered by
one allocation/RSS budget. Errors and outer transaction unwind restore objects,
page order, reverse indexes and the commit counter; revision invalidation still
requires snapshots to be queried again after rollback.

The independent gate is `docs/check-v010-structure-transfer.py <new-directory>`.
Build `transfer_structure` first and use the existing Python 3.11/pikepdf, qpdf and
MuPDF environment. It follows copy, reorder, delete, body edit and cross-document
import, checks pointers and source preservation, strictly reopens the output and
observes MuPDF text. It does not produce pixel or conformance conclusions.
Current test evidence is in the [v0.10 verification record](releases/v0.10-acceptance.md).
