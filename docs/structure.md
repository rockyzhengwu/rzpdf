# Structure inspection and content ownership

v0.10 provides two read-only entry points: `structure_tree()` inspects declared
hierarchy, qualified roles and local attribute attachments;
`structure_associations()` additionally validates supported
content ownership. Neither is a semantic text extractor or a PDF/UA conformance
checker. The remaining work is tracked in the
[v0.10 plan](releases/v0.10.md).

Explicit node authoring and page-content marking have a separate transactional
API; see the [structure editing guide](structure-editing.md). Read support does
not imply that every inspected extension can be safely edited.

```rust,ignore
if let Some(tree) = doc.structure_tree()? {
    for element in &tree.elements {
        println!("{} -> {} in {}", element.role,
            element.resolved_role.name, element.resolved_role.namespace_uri);
    }
    assert!(doc.is_stamp_current(&tree.stamp));
}
```

Missing StructTreeRoot returns `None`; an existing empty root returns `Some` with
no elements. The root and structure elements must be indirect for this initial
addressable model. Elements are returned in declared K preorder, not paint order
or an inferred reading order. Root children and each element's children preserve
their relative order. P must match the declaring parent; hierarchy cycles and
shared element ownership are errors. Pg, when present, must identify a live page.

Structure-element Type may be omitted. Raw role names and mapping endpoints
are distinct. `mapped_role` retains the legacy root RoleMap endpoint for elements
without NS; `None` means a permitted cyclic association or an explicitly
namespaced element. `resolved_role` distinguishes them with qualified namespace
identity and an Endpoint/Cycle status. Neither endpoint certifies that the name
is a known standard role or that the hierarchy conforms to Tagged PDF.

Title, Lang, Alt and ActualText are decoded as bounded PDF text strings. They are
the element's own entries, not computed language inheritance or replacement text
for rendered content. This API does not change existing paint-order extraction or
enable ActualText-bearing BDC in the default interpreter path. Logical replacement,
Artifact filtering and declared-order extraction have separate opt-in APIs; see
the [logical text guide](marked-text.md).

`StructureKid::UnvalidatedContent` deliberately carries original integer MCIDs
or MCR/OBJR dictionaries/references. `unvalidated_parent_tree` is likewise raw.
The shallow hierarchy API does not validate MCID scope, page/Form association, referenced
objects, ParentTree slots or correspondence to actual content operators. Raw PDF
references belong to the source document/session; do not transplant them into a
different document. Re-query after reopen and after any change invalidating the
tree's DocumentStamp. Unknown dictionary entries remain in the original objects;
this inspector never rewrites them.

Limits cover hierarchy depth, total child entries, RoleMap count/chain depth,
name tokens and cumulative decoded metadata/role bytes. This is not a total
allocation/RSS guarantee. IDTree, Artifact filtering and optional content remain
outside this model. Supported [structure-aware transfers](structure-transfers.md)
have separate validation and transaction rules; inspection alone does not make
unknown extensions or ownership changes safe to edit.

## PDF 2.0 namespaces

Namespace syntax requires effective PDF 2.0 (header or Catalog Version). The
reader never upgrades the document. Namespace dictionaries must be indirect,
have a nonempty NS text string and, if Type is present, Type Namespace. An
element's explicit NS must be listed in the root Namespaces array; NS is not
inherited. Transitive RoleMapNS targets and attribute-only namespace dictionaries
can be unlisted. `tree.namespaces` exposes the listed flag, URI, handle, raw
Schema and role mappings. Schema is retained without fetching any external data.

Root RoleMap applies only to elements without explicit NS. RoleMapNS targets
are either a name in the default standard namespace (`http://iso.org/pdf/ssn`),
or `[name namespace-reference]`. Cross-namespace chains preserve both role name
and namespace identity, so identical names in different namespaces do not collide.
Cycles or re-entry into the same explicit namespace fail, including unused
mappings. Legacy root RoleMap cycles remain inspectable with Cycle status.
Unknown namespace/role endpoints remain explicit rather than being relabelled as
standard types. This limited model rejects multiple namespace dictionaries for
one URI instead of choosing between potentially conflicting definitions. It does
not validate standard role vocabularies, schemas or Tagged parent-child rules.

## Attribute classes and local precedence

`tree.attribute_classes` exposes ClassMap; each element retains R (default 0),
C class uses and expanded `attribute_bindings`. Bindings retain their revision,
direct-A or class source, owner, raw values and optional opaque stream handle.
A/C revision pairs must be nonnegative integers no greater than element R;
omitted revisions default to zero. Older revisions remain visible, not silently
filtered or asserted current. An undefined class is empty and has `defined=false`;
a ClassMap empty array is an explicitly defined empty class, not an empty A array.

Use `element.attribute(&owner, key)` for a local lookup:

```rust,ignore
use pdf::document::structure::{StructureAttributeLookup, StructureAttributeOwner};
let owner = StructureAttributeOwner::Name("Layout".into());
match element.attribute(&owner, "TextAlign") {
    StructureAttributeLookup::Value { value, binding } => {
        println!("{value:?}, revision {}, source {:?}", binding.revision, binding.source);
    }
    other => println!("{other:?}"), // Missing, Ambiguous or Opaque
}
```

Direct A overrides C per owner and key, not by replacing an entire owner's
attributes. PDF 2.0 uses later-entry precedence within class and direct arrays;
PDF 1.x duplicate owner/key attachments report Ambiguous because those arrays
have no inherent precedence. Ordinary owners use their O name. An NSO owner
requires PDF 2.0 and indirect NS; its identity is the namespace URI, separate
from ordinary O names. Attribute streams are retained without decoding unknown
payload formats and yield Opaque for potentially affected lookups. Unknown values
and references remain raw, document-scoped data.

These are local attachment rules, not layout inheritance, default computation or
validation of every standard/private attribute value. Inline values, repeated
class expansion and namespace/mapping work share bounded entry and text limits;
unknown references and opaque stream payloads are not recursively interpreted.

## Content ownership validation

`doc.structure_associations()?` returns `None` without a tree, or a stamped
`StructureAssociations` snapshot. Its `items` preserve depth-first declared K
order, including interleaved element and content children. Each item identifies
its owning element and page with document-scoped handles:

- `MarkedContent` identifies a `StructureContentDomain::Page` or `Form`, plus
  its MCID. Equal MCIDs in different domains are independent. A page's Contents
  array is one domain, including sequences spanning adjacent streams.
- `ObjectReference` identifies an annotation on the declared page or an XObject
  reached by a Do invocation. Whole XObjects used on multiple pages require an
  OBJR on each page; repeated invocation on one page needs only one reference.

Validation scans actual operators, following invoked Forms rather than treating
every resource as used. It resolves named BDC Properties, checks MCID uniqueness
within each domain and marked-content balance, and matches integer K/MCR/OBJR
against ParentTree in both directions. It checks StructParent(s) key uniqueness,
required keys, null array holes, slot bounds and owners, and ParentTreeNextKey
when present. Content Pg overrides the element's own Pg; ancestor Pg is not
silently inherited. A Form without Resources uses the page's resources (the
obsolete PDF 1.x fallback), not the immediate parent Form's resources. An explicit
empty Resources dictionary remains empty.

Shared Forms are rescanned in each page context under cumulative work limits.
If resource lookup changes their MCID set, validation rejects the ambiguity.
Recursive Forms, duplicate declarations, missing owners and incoherent reverse
slots fail without editing the document. Logical limits cover cumulative decoded
content bytes, invocation/association/slot entries, operations and nesting; they
are not an aggregate allocation/RSS budget. Scan errors include the page/content
domain when available. Strings, comments and inline image data are parsed, not
searched for operator-like text.

The supported ownership subset excludes appearance-stream MCR, StmOwn and IDTree.
Qualified roles and local attributes use the model above; complete PDF 2.0 content
restrictions are not a conformance check here. ParentTree owners outside the
page/invoked-Form/annotation subset are explicitly unsupported; the API is not a
general Tagged PDF conformance verdict. MCIDs in scanned content require declared
K owners, and each domain/MCID has one declaration. Unknown attributes and other
dictionary extensions remain untouched in stored objects. This does not activate
ActualText replacement, Artifact filtering, optional-content evaluation or new
rendering behavior. Use the [logical text APIs](marked-text.md) to request those
text semantics explicitly. `structure_tree()` remains available for shallow inspection
when strict ownership validation cannot handle an input.

Run the slice regression with:

```sh
cargo test --locked -j2 -p pdf --test v010_structure --test v03_marked_content \
  --test v010_structure_associations --test v010_structure_semantics \
  --test v09_navigation --test v09_page_relationships
cargo build --locked -j2 -p pdf --example inspect_structure
cargo build --locked -j2 -p pdf --example validate_structure_associations
cargo build --locked -j2 -p pdf --example inspect_structure_semantics
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 \
  docs/check-v010-structure.py <new-output-directory>
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 \
  docs/check-v010-associations.py <another-new-output-directory>
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 \
  docs/check-v010-semantics.py <third-new-output-directory>
```

The independent gate needs existing pikepdf and qpdf. It checks an independently
authored hierarchy, read-only source behavior, strict reopen and rewrite retention
of raw content/metadata. It does not perform rendering or validate ParentTree
coherence. The associations gate separately creates a two-page/shared-Form
fixture, checks ownership, null slots, strict roundtrip and pointer/content
preservation, and rejects two syntactically valid but semantically broken inputs.
The semantics gate independently creates PDF 1.7/PDF 2.0 fixtures, compares
qualified endpoints and local precedence, checks MCID/ParentTree integration and
strict preservation, and rejects cyclic/unlisted namespaces and invalid revisions.
These gates do not verify replacement-text extraction or visual output. No Poppler
or dependency installation is needed.

Basis: [ISO 32000-1:2008, 7.8.3 and 14.7](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf),
[PDF 2.0 chapter 14 errata](https://pdf-issues.pdfa.org/32000-2-2020/clause14.html),
and the PDF Association's [namespace model](https://github.com/pdf-association/arlington-pdf-model/blob/master/tsv/latest/Namespace.tsv)
and [RoleMapNS model](https://github.com/pdf-association/arlington-pdf-model/blob/master/tsv/latest/RoleMapNS.tsv).
