# Annotations and appearance rendering

The v0.9 stage B APIs add typed markup CRUD without modifying page Contents.
They build on the document's identity, permission, transaction and Form execution
machinery. No actions, scripts or embedded files are executed.

## Supported markup

`document::annotations` exports `Annotation`, `AnnotationKind`, `AnnotationRecord`,
`TextAlignment` and `AnnotationRemoval`. Supported kinds are Square, Circle, Text,
Popup, FreeText, Line, Polygon, PolyLine, Highlight, Underline, Squiggly, StrikeOut,
Ink, Stamp and FileAttachment. The old Square add/read APIs use the same model.

```rust
use pdf::document::annotations::{Annotation, AnnotationKind, AnnotationRemoval};

let mut note = Annotation::new(
    AnnotationKind::Text { icon: "Note".into(), open: false },
    [20.0, 40.0, 50.0, 70.0],
);
note.contents = "Review this paragraph".into();
let handle = doc.add_annotation(0, &note)?;
note.contents = "Reviewed".into();
doc.update_annotation(0, &handle, &note)?;
doc.remove_annotation(0, &handle, AnnotationRemoval::RejectRelated)?;
```

`page_annotations` returns all entries, including Link, Widget and unknown types.
The typed `annotation` is None for kinds or variants outside this editable subset;
`unsupported_reason` explains a recognized but unsupported variant. The original
object remains addressable and is not replaced. Malformed supported structures
still return errors. Direct annotation dictionaries have no stable editable handle.
Handles belong to one open document; obtain new handles after reopening a file.

Positions, vertices, line endpoints and QuadPoints are in unrotated default PDF
user space. Rect must have finite positive dimensions of at least one unit.
Coordinates must lie within Rect. Text-markup quads accept Adobe Z order or convex
perimeter order; zero-area/self-intersecting quads are rejected. Borders are solid;
closed shapes optionally have RGB interior color. Stroke/icon C colors are
Gray/RGB on read and RGB on write; interior IC colors are RGB. CMYK,
transparent color arrays and advanced border styles
are outside the editable subset, not silently converted.

Creation uses a conservative PDF 1.4 minimum, or 1.5 for Polygon/PolyLine. It does
not automatically change the document version. Replies require 1.5; ordinary
replies omit RT and use its default R value (explicit RT starts in 1.6).
ToggleNoView requires 1.5 and LockedContents 1.7. Known annotation version
requirements are also checked during saving after low-level edits.
Flags default to Print. ReadOnly,
Locked or LockedContents prevent edits. Unknown keys survive updates, but known
visual properties and normal AP are replaced from the supplied model. Rollover
and down AP entries survive. Regenerating a model appearance is not a lossless
round-trip of arbitrary hand-authored artwork or stateful markup appearances.
Additional actions, optional-content/tag ownership, rich text, grouped replies,
legacy/dashed borders, line endings and measurement/callout features are refused
on edit rather than partially rewritten.

## Text and files

Text and FileAttachment use deterministic self-contained icons. Popup's Open
state and metadata are stored, but the core does not draw a viewer's interactive
popup window. Stamp uses a plain Courier label and frame, not an application icon
library. Contents metadata supports Unicode.

Legacy FreeText appearance generation uses plain ASCII Courier, explicit LF line
breaks, font size and left/center/right alignment. For explicit static TrueType
Chinese/Latin appearances, use the [Unicode FreeText APIs](unicode-freetext.md):
create, save/reopen and update with persistent font bindings. Oversized text is
rejected; neither profile silently wraps or shrinks. Rich text, complex shaping
and arbitrary external DA/font appearance editing remain unsupported.

Use `add_attachment` and `attachment_filespec` to obtain a Filespec handle for
`AnnotationKind::FileAttachment`. `annotation_file_data` decodes an annotation's
embedded payload without launching or opening it. Decode limits still apply.
Removing an annotation does not erase the Filespec or its shared data.

## Popup and reply relationships

`set_annotation_popup(parent, Some(popup))` maintains both Popup and Parent.
Pass None to detach. A Popup must be on the same page, have a supported editable
parent, and cannot belong to two parents. `set_annotation_reply` maintains IRT/RT
for supported same-page markup. Self-links, cycles, missing ownership and excessive
depth fail atomically. Cross-page/grouped relationships remain outside this subset.

Removal policy is explicit. RejectRelated refuses owned Popups or replies;
Cascade removes the subtree of owned Popups and replies from page Annots. Every
affected annotation must be editable. Removing a Popup alone clears its parent's
Popup link. All edits roll back together on errors. Shared page ownership is
rejected. Unlinked objects remain in the document: this is not redaction or secure
erasure. Copy/import/delete of complete annotation/field graphs remains V09-10.

## Optional rendering

`PdfPage::display` and `display_list` remain Contents-only. Use
`display_with_annotations(device, options)` or `display_list_with_annotations` to
opt in. `AnnotatedDisplayList::contents()` returns only original page content;
annotation lists are private and their drawing sources are cleared before replay,
including when the destination is a TraceDevice. Text extraction and content
editing therefore retain their original meaning.

`AnnotationRenderOptions` selects Screen/Print, Widget inclusion and `view_zoom`.
Screen observes Hidden/NoView; Print requires Print and ignores NoView. Invisible
only suppresses unknown subtypes. Popup UI is omitted. NoRotate counter-rotates
around Rect's upper-left point. NoZoom compensates explicit viewer magnification,
not physical device density. Set `view_zoom` to the viewer magnification and scale
the output device accordingly; the default is nominal 1x. NoZoom is ignored for
Print. ToggleNoView's interactive hover behavior and action execution are not part
of static rendering.

Normal AP streams are selected through AS when N is a state dictionary; missing
or inconsistent states are errors. BBox is transformed by AP Matrix, fitted to
Rect and executed as a Form, retaining clipping, local resources and transparency.
Omitted Form resources inherit the page scope without replacing existing names.
Visible non-Link markup without AP, invalid APs and unsupported optional-content
visibility fail explicitly. Links without AP are not painted; this path does not
synthesize legacy Link borders. Hidden entries do not decode their APs.

New APs are self-contained. Annotation opacity applies to the entire isolated
artwork, preventing repeated darkening at Ink intersections. Highlight uses
Multiply so that underlying black text remains black. Shared assembly is also
used by the existing AcroForm AP generator. Interpreting Contents and all APs shares
logical operation/decoded/path/text limits; retained display totals are checked
across layers. This is not a promise that all legacy allocations are attached to
one live-memory pool. Preflight precedes device replay; callers must discard a
partially painted output if the device itself fails during replay.

## Validation

`pdf/tests/v09_annotations.rs` covers all kinds, strict reopen, CRUD, relationships,
rollback, limits, provenance and R6 permissions. Native raster tests cover all four
NoRotate rotations with nonzero BBox/AP Matrix and NoZoom at 1x/2x.

Build `v09_annotations` and `render_annotations`, then run
`docs/check-v09-annotations.py <new-output-directory>` with the existing
pikepdf/Pillow environment. The gate uses qpdf, independently produced PDFs, MuPDF,
Skia and numeric compositing assertions. No font downloads or Poppler are needed.
The retained diagnostic documents a MuPDF 1.27 NoRotate discrepancy for nonzero
BBox plus rotated AP Matrix: our canonical and equivalent AP renderings match,
while that MuPDF combination differs. The primary comparison does not hide the
discrepancy by increasing its tolerance.

Normative basis: [ISO 32000-1, 12.5.3 and 12.5.5–12.5.6](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf).
