# Page relationship operations

Page indices refer to the order before an operation. Object handles identify
objects in one document session; reacquire them after save/reopen. Page edits
are transactional, including field publication and name-tree changes.

## Copy and import

For an unstructured document, `copy_page` shares existing content/resources but clones supported annotations
and field ownership. A copied field becomes an independent logical field: filling
it does not change the original. Supply a unique prefix when names would collide:

```rust,ignore
use pdf::document::page_transfer::PageTransferOptions;

doc.copy_page_with_options(0, 2, &PageTransferOptions {
    name_prefix: "copy".into(),
    ..Default::default()
})?;

target.import_pages_from_with_options(&source, &[0, 1], 0,
    &PageTransferOptions {
        name_prefix: "imported".into(),
        include_navigation: true,
        preserve_labels: true,
    })?;
```

Supported declared structure now migrates automatically through these APIs.
Structure copies isolate the selected resource graph and structural Form domains;
projection, replacement boundaries and conflict policies are documented in the
[structure transfer guide](structure-transfers.md). This does not make arbitrary
Tagged extensions editable.

Batch import uses one object-ID map, preserving shared fonts, AP streams and
cross-page references between selected pages. References to unselected source
pages reject the import. Same-document copy remaps self-targets to the copy and
keeps links to other existing pages. The source document is unchanged by import.
Encrypted sources and sources newer than the target's effective PDF version
remain outside the import boundary.

Supported markup annotations retain AP, resources, flags, metadata, file payloads
and unknown extension values. Popup/Parent and IRT relationships must be valid,
reciprocal where required, and on the same page. Widget P/Parent/Kids and the
AcroForm Fields tree are remapped together; inherited values and styles survive.
Text/Choice and ordinary Checkbox fields can retain only their selected widgets.
Partial Radio or indexed-button `/Opt` membership is rejected; selecting all their
widgets preserves V/AS/AP without renumbering states. A retained Checkbox subset
must still represent its logical value.

Names never merge implicitly. A nonempty prefix changes each imported root field
and named destination from `original` to `prefix_original`; child partial names
remain unchanged. Any remaining collision fails the whole operation. AcroForm DR
resources merge only when names are disjoint or resolved objects are identical;
conflicting font/resource names reject rather than silently changing DA meaning.
Independent but equivalent embedded-font graphs can therefore still conflict.

Catalog navigation is opt-in. With `include_navigation: true`, the complete source
outline and named-destination set is copied, all targets must be selected, and
hierarchy, open state, views and supported extension values are retained. The
default leaves that catalog navigation behind; imported named Link targets become
explicit destinations, avoiding dependencies on omitted names. Document metadata,
attachments in the catalog and other catalog features are not implicitly merged.

Explicit source labels migrate by default, including selection order. Existing
target labels remain page-bound. `preserve_labels: false` gives inserted pages
ordinary physical decimal labels when an explicit target label tree exists.
Without any explicit labels, normal PDF physical numbering remains implicit.
Unknown label-range or label-tree root extensions block automatic remapping,
rather than being dropped when rebuilding the label tree.

## Delete and reorder

`delete_page` rejects navigation targeting the removed page, not merely the
presence of navigation elsewhere. Choose an explicit surviving target to redirect
links, outlines and named destinations while keeping their view parameters:

```rust,ignore
use pdf::document::page_transfer::PageDeletionPolicy;

doc.delete_page_with_policy(0,
    PageDeletionPolicy::Retarget { page_index: 1 })?;
```

All annotations on the removed page detach together. Fields lose departing
widgets; fields with no surviving widgets and newly empty ancestor groups unlink
from Fields/Kids. Surviving field/widget identities and values stay unchanged.
Deleting only part of a Radio or indexed-button field is refused. A bounded scan
of retained live trailer roots, including Catalog, Info and private trailer
graphs, rejects unknown back-references to the removed page. Reached XRef/ObjStm
containers also reject because their opaque payload relationships cannot be
rebound by this operation. Ordinary source indexing alone does not count as a
business reference to a container.
Detached historical objects may remain in the file, including an annotated page
without a page-tree Parent; deletion is not secure erasure or reachability cleanup.

`reorder_pages` preserves page IDs, navigation targets, annotation membership and
Widget P references; explicit labels follow their pages. Effective inherited
page resources/boxes/rotation are materialized when moving across tree parents.

## Safety boundary and verification

The [object preservation matrix](object-preservation.md) separates raw saving
from page ownership changes, including page AF/AFRelationship, raw XMP,
ordinary shared extensions, private trailer roots and source containers.
Cross-document import rejects structural XRef/ObjStm containers in a page or
resource extension graph rather than copying stale encoded IDs or offsets.

Page operations refuse unsupported ownership/action graphs, including unsupported
Tagged extensions,
article threads, direct/shared annotation ownership, additional/chained actions,
non-Link annotation actions, unknown annotation kinds, and locked annotations.
Field transfer/deletion also refuses calculations, XFA, signature flags, unknown
form extensions, reader-generated appearances, read-only fields and unsupported
field variants. These are conservative errors, not silent removal. No action,
script, file or network operation is executed. Existing version, assembly,
modification, annotation and signature guards remain in force.

Graph depth/count, imported token/stream/payload sizes and label expansion are
limited. The existing non-budgeted APIs do not promise a total RSS or whole-engine
allocation limit. Rollback restores objects, allocation IDs and `commit_revision`;
the internal `revision` still advances to invalidate stale snapshots.

Tests: `cargo test --locked -j2 -p pdf --test v09_page_relationships`.
The independent gate is `docs/check-v09-page-relationships.py <new-directory>`
with the existing Python pikepdf/Pillow environment, qpdf and MuPDF. Build
`v09_page_relationships` and `render_annotations` examples first. It exercises
import, copy, retarget/delete, reorder and strict reopened refill, validates both
local and independently authored structures, requires identical same-engine
pixels before/after migration and compares final pages with MuPDF.
