# Layer authoring

rzpdf provides explicit creation and editing of a selected PDF optional-content
profile. It does not infer layers, execute SetOCGState actions, flatten content,
or remove hidden data. The existing [selection APIs](optional-content.md) remain
read-only; authoring is a separate operation.

## Create and configure

```rust,ignore
use pdf::{budget::OperationBudget, document::{Document,
    optional_content_edit::{LayerIntent, LayerBaseState, LayerConfiguration}}};

let doc = Document::new()?;
doc.add_page(0)?;
let budget = OperationBudget::new(64 * 1024 * 1024);
let layer = doc.create_layer("Review notes", LayerIntent::View, &budget)?;
let target = doc.layer_configuration_target(None)?;
doc.write_layer_configuration(Some(&target), &LayerConfiguration {
    name: Some("Default".into()), base: LayerBaseState::On,
    on: vec![layer.clone()], off: vec![],
}, &budget)?;
doc.append_layer_content(0, &layer, b"1 0 0 rg 20 20 40 40 re f", &budget)?;
let bytes = doc.save_to_bytes()?;
```

`layer_configuration_target(None)` addresses the default dictionary;
`Some(index)` addresses an alternative at the current revision. Passing `None`
as the *write target* instead appends an alternative. A target is not a mutable
dictionary handle: any edit or entered-and-rolled-back transaction invalidates
its stamp. Reacquire it after edits. Names may repeat and never select identity.

Configuration input supplies complete Name/BaseState/ON/OFF fields. An absent
name removes that field. Existing Creator and Intent are retained. Configurations
with dependent UI/usage rules or unknown extensions refuse editing instead of
silently dropping those keys. A default configuration must use BaseState ON
and Intent View (or its implicit default); OFF and Unchanged are alternative-only
generation choices. Existing permissive reads are not tightened by this rule.
To start with all current groups hidden, keep BaseState ON and list those groups
in OFF. New configurations use the default View intent; this profile does not
provide an Intent editor for authoring Design configurations.

## Membership and invocation scope

`create_layer_membership` creates an OCMD with a nonempty group list, one of four
policies, and an optional typed visibility expression. Expression leaves must be
registered groups in this document. VE takes precedence over P; unsupported
intent combinations still refuse in the existing evaluator. OCG requires PDF
1.5; VE requires 1.6. A low-version document must be explicitly upgraded through
the existing save/reopen workflow, not by the editing call.

`append_layer_content` validates a self-contained textual source segment and the
old page program before appending a new stream. The new stream has a q/Q and
OC BDC/EMC wrapper. Resource references must resolve and interpretation must
succeed; failure rolls back both the content and Properties registration.
Inline images (including ASCII-encoded BI segments), compatibility sections, new marked-content points,
new MCIDs and named BDC properties in supplied segments are not supported. New
Span/Artifact direct properties and ActualText are selected supported nesting.
The new stream inherits the page's existing graphics state; balanced q/Q does
not undo earlier clipping. No automatic normalization of old content is implied.

`bind_layer_xobject_call(page, expected, operation, membership, budget)` takes a
current page `ContentProgram` and its original operator index, not a screen-hit
or collection index. It wraps exactly one top-level Form/image Do in BDC/EMC;
the shared XObject is not mutated or cloned. Other calls and pages retain their
original bindings. Existing outer OC remains an additional condition. Targets
with their own OC or structure ownership are refused; nested calls require a
separate future API. Wrapping a call is not a resource-wide layer assignment.

Hiding paint still executes state operations, clipping and text advances under
the original evaluator's rules. Hidden content remains in saved bytes. For
preview and text extraction, explicitly obtain a new visibility selection after
editing; old visibility snapshots and content programs are stale.

## Limits and protection

The first profile caps groups at 1024, alternatives at 256, expression nodes at
4096, expression depth below 64, and each validated segment/page program at
1 MiB. Parser/object/work limits can lower applicable ceilings. Every writing
API takes one shared budget and uses conservative scratch reservations plus
tracked owned dictionaries, content and transaction snapshots. The scratch
minimum is 2 MiB, not an actual allocation measurement. Existing read models,
interpretation allocations, source caches and native resources are not all
tracked; this is not an RSS limit.

Ordinary source/permission guards still apply, including signature-source
refusal and R6 modification permissions. Read-only recovery does not expose
these methods. Full and unsigned incremental saving use existing layouts and
protection defaults; no options fields or default layouts change. Removal,
merging, cross-document migration, flattening, and secure erasure are excluded.

See the [release boundary](releases/v0.13-boundaries.md) and
[acceptance record](releases/v0.13-acceptance.md). Build `v013_layers` for an
executable two-page authoring/roundtrip example and `v013_benchmark` for the
fixed-size baseline. The independent recipe is `docs/check-v013-layers.py`;
it uses existing pikepdf/qpdf/MuPDF and the explicit Skia layer renderer.
