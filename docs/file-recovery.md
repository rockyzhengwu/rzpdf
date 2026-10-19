# File diagnostics and read-only recovery

Ordinary parsing and index reconstruction are different capabilities. `Document::from_bytes` remains Compatible by default; `from_bytes_with_mode(..., ParseMode::Strict)` does not reconstruct a damaged cross-reference index. Neither mode certifies PDF conformance.

## File diagnostics

`Document::structure_diagnostics()` returns a bounded snapshot of file-structure events. `read_provenance()` records Created or Parsed(Strict/Compatible) from immutable construction state. Editing a trailer or saving with different output options cannot relabel that source.

Events include missing binary header comments, invalid final EOF, loose classic xref entry formatting, relaxed table constraints, missing XRef stream Type, extra decoded xref bytes, duplicate dictionary keys, stream-boundary searches and nonstandard stream-start whitespace. Each event has a byte offset, an object ID when parsing an indirect object, and a fixed reason enum. No input-controlled diagnostic strings are retained. Re-reading the same event does not duplicate it.

Reports are lazy: opening a document does not force all stream bodies to load. Ask for a new snapshot after interpreting the pages of interest. File events remain separate from `DisplayList::diagnostics()`, which describes content interpretation.

A stream-boundary-search event records the existing Compatible heuristic, not proof that the original intended stream bytes were recovered. Ordinary Compatible saving retains its previous contract; explicit index recovery instead refuses uncertain stream extents. Header and xref inspection do not add a body-line allocation or a stricter whole-line size requirement to Compatible token parsing.

Each log is limited to 1,024 events and a 1 MiB tracked byte pool intersected with `Limits::max_operation_bytes`. Capacity and growth overlap are charged; exhaustion returns ResourceLimit instead of silently truncating the report. The snapshot is a separate, bounded owned copy. The limits do not bound all parser/native allocations or process RSS.

## Explicit recovery

```rust
use pdf::document::recovery::RecoveredDocument;

# fn example(bytes: Vec<u8>) -> pdf::error::PdfResult<()> {
let recovered = RecoveredDocument::from_bytes(bytes)?;
for page in 0..recovered.try_page_count()? {
    let display = recovered.display_list(page as u32)?;
    // Replay this detached display list using a rendering device.
    println!("{} x {}", display.page_width(), display.page_height());
}
assert!(!recovered.diagnostics().is_empty());
# Ok(()) }
```

The first profile reconstructs an index only for a complete, simple, unencrypted, single-revision file with an explicit unique trailer Root. It accepts a wrong startxref, a missing classic xref table, or a damaged table still consisting only of numeric/state tokens. It parses sequential indirect objects using the existing Strict object parser; it never scans stream data or strings for `obj`, `endobj` or `trailer` substrings.

The profile requires a valid header with its binary comment, exact stream boundaries with direct Length, unique object numbers, a consistent Size, an indirect Catalog Root, one trailer and the final startxref/EOF suffix. Forward/indirect Length, duplicate objects or keys, unknown bytes between objects, multiple revisions, hybrid, XRef/ObjStm objects, indirect ownership Type, encryption, signature/security markers and linearization are refused. The 64-byte EOF suffix cap is a conservative product policy. Refusal does not mean that every such input is invalid PDF; some are outside this recovery profile.

`RecoveryLimits` can lower the 64 MiB input and 100,000 candidate ceilings. Candidates also intersect `Limits::max_objects`; parsing retains token/depth/raw-stream limits. Byte reads, lookahead and stream payload work share at most `16 * input_length + 1024` steps, which can be lowered separately. Exceeding any limit fails; no partial recovered document is returned. `from_bytes_with_budget` accepts a shared OperationBudget for the retained input, conservative index growth and budget-aware object parsing. Tracked memory is not an allocator-level or RSS ceiling.

## Read-only capability

`RecoveredDocument` is intentionally not a `Document` and does not implement Deref. It exposes detached page geometry/display lists, Info and raw metadata, the report and provenance. There is no underlying context/page handle, conversion, mutation, full/incremental save, signature preparation or controlled-update API. Compile-fail doctests enforce the boundary. Passing Compatible in ParseOptions does not relax recovery's Strict object grammar.

The damaged original remains damaged. Recovery never writes a repaired PDF, changes ordinary Compatible behavior into automatic reconstruction, or creates a Strict immutable signing baseline. Applications may separately export detached text or rendered images; this does not imply authorization to rewrite the original object graph.

## Runnable example and verification

```sh
cargo build --locked -p render --example v012_recovery
target/debug/examples/v012_recovery damaged.pdf NEW_IMAGE_DIRECTORY recover
```

The example checks input size before reading and creates its output directory only after successful interpretation. It writes PNGs, never a recovered PDF. Compare with `strict` on the undamaged source. The [offline gate](check-v012-recovery.py) produces original pikepdf/qpdf/MuPDF samples, paired mutations, Strict refusals, exact same-renderer before/after pixels and independent MuPDF references. It records producers, public synthetic provenance and hashes. No Poppler or network access is required.

See also [advanced saving](advanced-saving.md), [object preservation](object-preservation.md) and the [version boundary](releases/v0.12-boundaries.md). Lexical and file-structure references: [ISO 32000-2 corrections, clause 7](https://pdf-issues.pdfa.org/32000-2-2020/clause07.html). Recovery limits are implementation policy, not standard-prescribed limits.
