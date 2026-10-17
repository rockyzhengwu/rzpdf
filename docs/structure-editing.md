# Explicit structure editing

V10-04 adds transactional authoring of the supported PDF logical structure graph.
Callers choose roles and reading order; the library does not recognize headings,
lists or tables from page layout. Existing paint-order extraction remains unchanged.
V10-05 adds bounded [structure-aware page transfers and body-edit validation](structure-transfers.md);
unsupported ownership and extensions still reject.

## Nodes and local metadata

```rust,ignore
use pdf::document::structure::{StructureElementDefinition, StructureRemoval};

let group = doc.add_structure_element(
    None, 0, &StructureElementDefinition::new("Document"))?;
let paragraph = doc.add_structure_element(
    Some(&group), 0, &StructureElementDefinition::new("P"))?;

let mut definition = StructureElementDefinition::new("CustomParagraph");
definition.title = Some("标题".into());
definition.language = Some("zh-CN".into());
doc.set_structure_role_mapping("CustomParagraph", Some("P"))?;
doc.update_structure_element(&paragraph, &definition)?;
doc.move_structure_element(&paragraph, Some(&group), 0)?;
```

`None` denotes the structure root; the first add creates it if absent. Indices
address mixed K order, including content items, not just element siblings.
Move indices are measured after detaching the element. Moves reject cycles,
foreign or detached element handles, bad parents and out-of-range indices.

Updates replace S, T, Lang, Alt, ActualText and local A attributes. Omitted text
fields remove their previous value; empty ActualText remains an explicit empty
replacement. Existing NS, C, R, Pg and children remain. `StructureAttributeWrite`
accepts an owner name and direct scalar/array/dictionary values; reserved O/NS,
duplicate owner/key pairs, indirect references, streams and nonfinite numbers
reject. New A attachments use the element's current R revision. C/ClassMap and
namespace definitions are preserved, not authored by this interface. Attribute
values are explicit data, not a layout engine or a full attribute-schema validator.

`set_structure_role_mapping(from, Some(to))` sets a legacy root RoleMap entry;
`None` removes it. Authoring rejects cycles. Existing qualified NS mappings still
follow the read model; an endpoint is not a standard-role validity certificate.

## Removing a node

`remove_structure_element(handle, StructureRemoval::EmptyOnly)` refuses children,
including content items. `PromoteChildren` removes only that node and inserts its
children at its old position. Child P links and ParentTree owners are updated;
content references carry explicit Pg before ownership changes. A root cannot own
promoted content directly, so that case rejects. The removed node's metadata and
replacement text are not inherited by the promoted children.

These operations detach nodes, not erase objects or redact painted content.
They do not provide recursive deletion of a content-bearing subtree. Existing
Form MCR and annotation/XObject OBJR associations can be retained or re-owned;
the corresponding Form/appearance bytes are not edited.

## Marking existing page content

```rust,ignore
let stamp = doc.stamp();
let program = doc.get_page(0)?.content_program()?;
// Choose complete original operation indices from program.operations().
let mark = doc.mark_structure_content(
    &paragraph, 0, &stamp, first_operation..end_operation, 0, Some("逻辑文本"))?;
assert!(doc.is_stamp_current(&mark.stamp));
```

The selection must be nonempty, unmarked, balanced and inside one original
Contents stream. A complete graphics object or a sequence within one text object
is allowed; partial paths, crossing enclosing q/Q or BT/ET, existing marked
sequences and cross-stream selections reject. Both original and resulting content
are interpreted under the supported engine model; diagnostics or unsupported
resources cannot be bypassed by supplying ActualText.

The writer inserts a Span BDC/EMC with a page-domain MCID. MCIDs append after the
existing ParentTree array, including null holes; new StructParents keys respect
ParentTreeNextKey and existing keys. An explicit MCR is inserted at the selected
element's K index. Optional ActualText is a bounded PDF text string, not encoded
glyph text. It does not change the original Tj/TJ bytes or invent positions.

A fresh page Contents stream isolates shared original streams and other pages;
unchanged lexical bytes and comments are retained. The page's Contents chunks are
consolidated. Forms themselves are not marked or rewritten through this API;
wrapping a Do belongs to the page domain. Existing Form structure domains remain
separate, and logical extraction retains its ambiguity checks.

## Transactions and limits

All writes require an unsigned source, modification permission and effective
PDF 1.4 or later. Older inputs require an explicit version upgrade and reopen.
Each operation validates supported ownership before editing, publishes within an
edit transaction, then validates the resulting graph. Errors and outer transaction
unwind restore objects and the commit counter. Handles allocated inside an outer
transaction must not escape before commit; requery snapshots after rollback.

Unknown structure/root/content-item or ParentTree-node extensions reject before
rewriting; the read-only inspector can still preserve and inspect broader inputs.
IDTree and reverse-reference extensions, appearance-stream MCR/StmOwn, new NSO or
opaque attribute authoring, and recursive content deletion remain unsupported.
The API does not set MarkInfo/Marked automatically or certify Tagged PDF/PDF-UA.

Hierarchy, object/slot counts, metadata, attribute entries/depth/bytes, content
operations, decoded bytes and role-map work are limited. These are not complete
allocation/RSS guarantees. See the [support matrix](releases/v0.10-support.md)
and [verification record](releases/v0.10-acceptance.md) for tested boundaries.

The independent gate uses the `edit_structure` example and
`docs/check-v010-structure-edit.py <new-output-directory>` with the existing
Python/pikepdf, qpdf and MuPDF environment. It checks structure output and logical
text, not pixels or full conformance. No Poppler or fuzzing is required.
