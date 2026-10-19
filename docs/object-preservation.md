# Object and relationship preservation

Saving preserves the supported PDF representation; it does not infer the meaning
of private schemas or certify an arbitrary file. Page operations have a narrower
contract because moving or removing an owner can change the meaning of a graph.
Use the tables below to choose an operation without silently dropping data.

## Saving a document

| Data | Full save | Unsigned incremental save | Semantic editing |
|---|---|---|---|
| Ordinary private dictionary keys, arrays and objects | Values and business object IDs retained, including sharing and reference cycles | Unchanged source bytes retained; changed objects use the same logical IDs | Raw object updates are construction tools, not a private-schema editor |
| Private trailer roots | Retained, except reserved layout fields rebuilt by the writer | Retained in the new trailer; original sections remain in the prefix | No inference about private ownership |
| Unreachable business objects | Not garbage-collected merely because they are unreachable | Remain in the source prefix and current object table unless explicitly deleted | Not secure erasure; detached history is not a live document root |
| Info and XMP | Unknown Info entries and raw decoded XMP packet bytes retained | The same, including after compression or R6 preservation | Info replacement is copy-on-write; Catalog XMP replacement does not synchronize page XMP or rewrite XML |
| EmbeddedFiles, Filespec, AF and AFRelationship | Dictionaries, names, reference sharing and embedded payloads retained | The same; no implicit removal of AF links | Attachment APIs manage the EmbeddedFiles name tree, not every association in AF |
| Supported structure and OCG/OCMD configurations | Existing graphs retained; no automatic repair or conformance upgrade | Metadata-only changes do not rebuild these graphs | See the separate structure and optional-content APIs and their limits |
| R6 protection and signatures | Existing authentication, version and source guards apply | Only the existing strict unsigned R6 subset | Preservation does not authorize signed rewrites or a protection downgrade |

"Retained" means the declared values, relationships and decoded payloads, not
dictionary order, physical offsets, whitespace or ciphertext equality. Raw XMP
packet bytes are compared after stream decoding; re-encoding and encryption may
change the on-disk stream bytes. Byte-exact original-prefix preservation is an
incremental-save guarantee, not a full-rewrite guarantee.

The internal trailer-root predicate is shared by serialization and page-deletion
checks. Prev, XRefStm, Type, W, Index, Filter, DecodeParms and Length are structural
fields, not independent business roots. Writers rebuild the applicable fields;
the live deletion scan also includes Info and retained private trailer roots,
not just Catalog.

## Source containers are not ordinary extension objects

The default Classic and independent-object XrefStream full profiles keep their
existing handling of old container data. This does not make an old XRef payload
a valid index for new physical offsets, or promise migration of opaque container
semantics. The new active index always comes from the selected writer layout.

Full object-stream repacking replaces only understood source XRef/ObjStm
containers without live ordinary references. Extends, unknown container fields
and ordinary/trailer references to replaced containers reject the operation.
An unknown field in an old XRef dictionary remains a refusal even when it also
appears in the source trailer; its meaning is not guessed from its name. A
private trailer field in a conventional source can be preserved in a newly
generated xref stream, but a later repack of that stream may conservatively reject
its unknown fields. This boundary is intentional and applies to this version.

Incremental saving does not rebuild or edit source structural containers, nor
generate new ObjStm containers. Modified compressed members become independent
objects with logical generation zero. Original container bytes remain valid
within their original section and Prev chain.

Cross-document page/resource import rejects XRef and ObjStm dictionaries,
including an indirect Type declaration. Their raw payloads encode member IDs or
physical offsets that a dictionary-only ID map cannot safely migrate. Ordinary
private fields do not bypass this refusal. Page deletion also rejects structural
containers reached through live business roots, rather than treating opaque
payloads as a proof that no owner reference exists. Containers reached only by
the file's xref machinery do not block ordinary page deletion.

## Copy import and delete

| Relationship | Same-document copy | Cross-document import or selection | Page deletion |
|---|---|---|---|
| Page private dictionaries and ordinary reference cycles | Direct page values copied; ordinary indirect graphs may remain shared | Reachable page graph cloned through one batch ID map; sharing/cycles retained | Live unknown references to the removed page reject |
| Cross-page references in private values | Existing indirect data is not heuristically rewritten; supported page links follow their documented copy policy | Selected page references remapped; unselected owner references reject | Retarget changes known navigation only, not arbitrary private references |
| Page AF, Filespec/EF, metadata and PieceInfo | Ordinary indirect data may remain shared | Data reachable from selected pages retained, including raw packets and payloads | Surviving associations and shared payloads retained; no automatic AF garbage collection |
| Source Catalog Info/XMP, attachments and private roots | Existing document state retained | Not implicitly merged; target Catalog metadata and names remain authoritative | Included in live-root checks where retained; unknown back-references reject |
| Supported navigation, annotations, fields and labels | Existing ownership/name policies | Existing batch mapping and explicit navigation option | Existing detachment/retarget rules; unknown or unsupported ownership rejects |
| Declared logical structure | Supported branches isolated and rebound | Supported projection and conflict rules | Supported pruning/rebinding only |
| OCG/OCMD configurations | Existing conservative transfer guards remain | Configuration migration is not supported | Unsupported configuration-dependent operations still reject |

Page import is not a whole-document merge. `merge_document` imports pages; it
does not merge every Catalog extension. Only the PDF-defined inherited page
attributes supported by the importer are materialized. Unknown page-tree
inheritance rules or external application state are not inferred.

An EmbeddedFiles name can be removed while a Filespec remains associated through
Catalog/page AF. Removing that name does not delete the Filespec or payload.
Likewise, replacing Catalog XMP leaves a formerly shared page packet unchanged.
There is no high-level general AF editor, XML synchronizer or private-schema
validator in this profile. Raw construction APIs can create invalid graphs;
successful serialization alone is not a relationship validation result.

## Failure and limits

Page operations use their existing transactions. Unsupported references and
payload/count/depth limits reject without partially publishing pages, names,
fields or allocator changes. Commit counters and serialized state are restored;
rollback can invalidate old stamps, so reacquire snapshots before retrying.
An import can fail for an unselected owner and then succeed with a valid complete
selection on the same destination.

The live-root deletion scan is bounded by max_objects and max_depth. Import has
its existing cumulative token/payload/raw-stream limits. These legacy page graph
operations do not promise that every temporary allocation is tracked by one
OperationBudget or that process RSS is bounded. Full/incremental writer budgets
and atomic host-file contracts are unchanged.

Deletion is unlinking, not secure erasure or whole-file graph cleanup. Detached
annotations and historical objects can remain. Private references in a retained
live root cannot be silently nulled or redirected to a different page.

## Verification and related guides

`pdf/tests/v012_preservation.rs` covers full/incremental layouts, R6, private
trailer roots, shared/cyclic data, raw XMP and AF, Catalog-only replacement,
import/delete refusals, rollback and same-document retry. The independent
`docs/check-v012-preservation.py` gate uses synthetic pikepdf/qpdf sources,
strict structural checks, graph projections and MuPDF page comparisons.

See [advanced saving](advanced-saving.md), [page relationships](page-relationships.md),
[structure transfers](structure-transfers.md), [optional content](optional-content.md)
and the [v0.12 acceptance record](releases/v0.12-acceptance.md). Standard context:
[ISO 32000 syntax errata](https://pdf-issues.pdfa.org/32000-2-2020/clause07.html)
and [qpdf's object-stream documentation](https://qpdf.readthedocs.io/en/stable/object-streams.html).
