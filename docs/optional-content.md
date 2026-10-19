# Optional content

rzpdf evaluates explicit PDF layers without changing or deleting their source
content. The core model is independent of Skia. Rendering and text filtering are
opt-in so existing paint-order APIs keep their original behavior.

## Select a configuration

```rust,ignore
use pdf::document::optional_content::OptionalContentOptions;

let model = document.optional_content()?; // None means no OCProperties
let visibility = document.optional_content_visibility(
    &OptionalContentOptions::default(),
)?;
let list = document.get_page(0)?.display_list_with_optional_content(&visibility)?;
list.replay(&mut device)?;
```

`optional_content()` exposes document-scoped group handles, decoded names, the
default configuration, and alternative configurations in `/Configs` order.
Original group/configuration dictionaries remain available for inspection.
This snapshot API is not an optional-content conformance validator or a
layer-authoring API. Selected creation and editing are available separately
through [layer authoring](layer-authoring.md).

`OptionalContentOptions` selects:

- `configuration: None` for `/D`, or `Some(index)` for an alternative. Names are
  display labels, not unique identities; duplicate labels do not select a group
  or configuration implicitly.
- `intent: View` or `Print` for viewer usage processing.
- `apply_usage: true` to process supported `/AS` rules. View rules initialize the
  viewer state; Print rules then apply to a temporary print selection. With
  `false`, only configuration states and explicit overrides apply, suitable for
  stand-alone printing or aggregation previews.
- `overrides` for explicit `(group_handle, bool)` state choices applied last.
  Foreign, unregistered, or duplicate overrides fail. These are complete state
  choices, not interactive radio-button toggles or SetOCGState actions.

Alternatives initialize from `/D`, then apply their own `BaseState` and `ON`/`OFF`
arrays. `Unchanged` therefore keeps the initialized default states; it does not
implicitly remember another viewer session. `ON`, `OFF`, and `Unchanged` are
supported. Conflicting ON/OFF membership is rejected. Common default dictionaries
with `BaseState OFF` are accepted; this does not certify their conformance to a
particular PDF edition.

`Usage` alone does not override a group. A matching `/AS` rule must request the
category. The supported categories are `View`, `Print`, and `Export` state
recommendations, within the selected View/Print events. Recommendations are
combined across all matching rules; absent usage entries leave the state alone.
Zoom, language, user, and unknown categories for a relevant event are rejected
instead of inferred from the machine running the library. Export events,
interactive actions, radio-button UI management, and persistent viewer sessions
are not executed. Order, RBGroups, Locked, and other known UI metadata are
preserved rather than treated as a rendered outline or interactive implementation.

## Membership and rendering

`OptionalContentVisibility::is_visible(document, membership)` accepts registered
indirect OCGs and direct/indirect OCMD dictionaries. It supports `AnyOn`, `AllOn`,
`AnyOff`, `AllOff`, and bounded `And`/`Or`/unary `Not` visibility expressions.
`VE` takes precedence over the legacy policy. Empty or null-only OCMD group lists
have no visibility effect. Unresolvable references fail; they are not silently
treated as deleted groups. Unknown policies/operators/extensions, unregistered
groups, malformed arity, reference cycles, and excessive work/depth fail.
All expression branches are validated, including branches whose result would
otherwise be short-circuited.

Configuration/group Intent matching controls whether a direct OCG affects
visibility. An unmatched direct OCG has no effect. OCMD/VE expressions involving
unmatched intents are outside the supported subset and explicitly fail; the
library does not guess how to simplify those expressions.

The explicit renderer supports named `/OC` BDC Properties, nested marked content,
Form/image XObject `/OC`, and annotation `/OC` combined with existing visibility
flags. Forms retain their resource scope, BBox, matrices, groups and invocation
sources. A missing Form resource dictionary falls back to page resources; an
explicit empty dictionary does not. Known Span/Artifact metadata is validated
without replacing painted glyphs with ActualText.

Hidden marked content is still interpreted and charged: text advances, colours,
transforms, path construction, clipping, q/Q and text-object state remain active.
Only paint dispatch is suppressed. Hidden Form/image programs are currently
validated too, so unsupported hidden content may still fail. Source operation
indices and decoded offsets are unchanged; collection indices are not source
operator indices. A filtered list is not a layer-flattened or redacted PDF.

Use `display_list_with_annotations_and_optional_content(options, visibility)`
for annotation/AP composition, and coordinate annotation Screen/Print options
with the layer selection. The existing annotation APIs continue refusing OC
without an explicit selection. Type3 marked-content glyph programs and an OC
entry on a soft-mask G Form remain explicitly unsupported. Marked content inside
supported pattern/mask streams receives the same selection; existing pattern,
mask and Type3 graphics limitations still apply. DP optional-content points are
not part of this initial interpreter subset.

## Text and provenance

`extract_marked_text_with_optional_content(options, visibility)` filters logical
paint-order spans. ActualText wholly inside a hidden sequence is omitted. An
outer atomic replacement containing hidden content fails instead of leaking text
or inventing a partition. Artifact filtering is independent of layer filtering;
hidden glyphs and replacements still consume work limits.

`extract_text_structure_order_with_optional_content(options, visibility)` keeps
declared `/K` order and validates the full ownership graph, including hidden
MCIDs. Structure-element ActualText with visibility filtering is conservatively
rejected when replacements are enabled. Set `use_actual_text: false` to request
raw mapped text explicitly; no replacement is silently discarded. Existing
shared-Form and cross-owner restrictions remain in force.

Snapshots are valid only for the current revision of their original document;
edits and rollback invalidate them. Each evaluation also has a distinct
`selection()` cache discriminator. Clones retain it and share the cumulative
evaluation budget. Display lists and logical reports expose this discriminator;
the document stamp alone cannot distinguish two configurations. Source/edit
provenance remains separate and unchanged.

## Safety and persistence

Selection is read-only: no source dictionary, permission, signature, or revision
is changed. It requires PDF 1.5 for optional content and 1.6 for VE. Saving a
lower-version graph with those features requires an explicit version upgrade.
Unknown raw objects remain preserved on supported full save; unknown visibility
rules are refused by evaluation. The counters bound logical configuration,
expression, metadata and content work, not every allocator/native allocation or
whole-process RSS.

Page copying/import/deletion/reordering involving OCProperties, and resource
imports containing OCG/OCMD or XObject OC, remain refused by the existing safety
guards. The read-only API does not migrate configurations, author layer dictionaries,
flatten layers, execute actions, or securely erase hidden content. Selected
authoring has its own [release boundary](releases/v0.13-boundaries.md). Low-level
object/stream construction remains available without these semantic guarantees.

Build `inspect_optional_content` and `render_optional_content` for command-line
examples. The independent gate is `docs/check-v010-optional-content.py`; it
generates its own pikepdf source, checks strict roundtrip/text and qpdf structure,
and compares three Skia configurations with independent MuPDF-rendered expected
geometry. Integer-aligned geometry must match; one RGB quantization level is
allowed for the two backends' colour rounding. The release [support matrix](releases/v0.10-support.md)
and [acceptance record](releases/v0.10-acceptance.md) define the verified scope.

The semantic basis is ISO 32000-1:2008 section 8.11, available from
[Adobe](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf),
with PDF 2.0 differences tracked separately in the
[PDF Association graphics errata](https://pdf-issues.pdfa.org/32000-2-2020/clause08.html).
