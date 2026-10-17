# Navigation and page labels

The v0.9 navigation APIs extend the v0.4 flat interfaces. They do not execute
actions or open URIs. Navigation is independent of page Contents and text editing.

## Destinations

`document::destination::{Destination, DestinationView}` represents an explicit
target. Construct one with `doc.destination(page_index, view)`. Its document-scoped
page handle stays bound across reordering; `destination_page_index` returns its
current position and rejects a missing page or foreign document. Handles are not
portable across save/reopen. Re-read destinations from the reopened document.

Views include XYZ, Fit, FitH, FitV, FitR, FitB, FitBH and FitBV. Optional coordinates
retain PDF nulls; XYZ zoom zero remains zero rather than becoming null. Coordinates
must be finite, zoom nonnegative, and FitR must have positive area.

`named_destination_views` reads complete targets. `named_destinations` is the
legacy page-index projection. `set_named_destination_view` preserves coordinates;
`set_named_destination` writes Fit. Named destinations and embedded files share a
bounded name-tree reader and balanced writer. Number trees use the same machinery.
Invalid ordering, duplicate keys, inaccurate Limits, cycles, shared children and
depth/count excesses return errors. Unknown keys on child tree nodes are rejected
because rebalancing could otherwise discard them. Root extensions are retained.

## Outlines and links

`outline_items` returns a preorder list with stable item and parent handles.
`add_outline`, `update_outline`, `move_outline` and `remove_outline` maintain
Parent/Prev/Next/First/Last and visible Count. A move index is evaluated after
detaching the subtree from its old siblings. Moving under a descendant is an error.
Removing an outline detaches its entire subtree; it is not secure erasure.

`page_links` reads links; `page_link_handles` gives their editable identities.
`add_link`, `update_page_link` and `remove_page_link` provide CRUD without using a
list index as identity. Updates preserve unknown keys, border, flags and AP.
Read-only/locked links and shared page ownership are rejected. Direct annotations
can be read but have no editable object handle. `LinkTarget::Destination` is the
full-view variant; the old Page, Named and Uri variants remain available. Adding a
floating-point destination means `LinkTarget` now implements PartialEq, not Eq.

GoTo and URI link actions are read, never executed. URI creation is restricted to
printable ASCII http, https and mailto. Additional/chained actions are outside the
editable subset. The legacy catalog Dests dictionary is not supported.

`rename_named_destination` changes the name-tree key and supported annotation and
outline GoTo references atomically. Collisions, catalog OpenAction/additional
actions, page or annotation additional actions, chained actions and direct
references that need rewriting are rejected. Removing a referenced name is an
error, not an implicit cascading deletion. All edits honor document permissions
and use the existing transaction/signature guards.

## Page labels

`page_label_ranges`, `set_page_label_ranges` and `page_label` expose PageLabels.
Ranges start at physical page zero and increase strictly. Styles are decimal,
upper/lower Roman and upper/lower alphabetic, or prefix only. Alphabetic numbering
follows PDF's A…Z, AA…ZZ, AAA…ZZZ scheme, not spreadsheet column numbering.

Explicit labels move with pages during reorder, retain their values when other
pages are inserted/deleted, and duplicate when a page is copied. A newly inserted
page receives its new physical decimal label when the target has explicit labels.
Import preserves explicit source labels by default, with an option to use physical
labels instead. A document without PageLabels keeps default
physical numbering. Unknown label-range or tree-root extensions prevent automatic remap
rather than being dropped; numeric overflow and excessive output text are errors.

Copy, import, delete and reorder maintain the supported navigation, annotation
and field ownership graphs. Name conflicts, unselected targets, deletion retargeting
and conservative refusal boundaries are explicit; see [page relationships](page-relationships.md).

## Verification

Run `cargo test -p pdf --locked -j2 --test v09_navigation` and the document unit
tests. The independent gate is `python3.11 docs/check-v09-navigation.py <new-dir>`
with the existing pikepdf/Pillow environment, qpdf and MuPDF. It generates its own
corpus, checks structures with independent readers, reads/edits independently
produced nested trees and GoTo actions, and checks three reordered raster pages.
No downloaded PDFs, fonts or Poppler installation are required.
