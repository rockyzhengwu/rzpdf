# Logical text and declared structure order

Two opt-in APIs extract PDF-declared text semantics without changing existing
paint-order character/run extraction or rendering:

```rust,ignore
use pdf::page::marked_text::MarkedTextOptions;

let options = MarkedTextOptions::default();
let report = doc.get_page(0)?.extract_marked_text(options)?;
let text = report.text(&Default::default())?;

match doc.extract_text_structure_order(options)? {
    Some(structured) => println!("{}", structured.text(&Default::default())?),
    None => println!("No declared structure tree"),
}
```

`MarkedTextOptions` defaults to `use_actual_text=true` and
`include_artifacts=false`. To inspect original glyph mappings including artifacts,
use `{ use_actual_text: false, include_artifacts: true }`. Calling this new API is
the opt-in: legacy strict extraction still rejects rich BDC properties and legacy
compatible reports retain their existing diagnostics. No default output changes.

## ActualText and artifacts

Page extraction resolves direct BDC dictionaries and named Properties resources,
checks BMC/BDC/EMC balance and text/graphics-state nesting, and follows supported
Form invocations. MCIDs remain scoped to their page/Form. An explicit empty Form
Resources dictionary stays empty; omitted Resources falls back to the page, not
the immediate parent Form.

An ActualText PDF text string on Span or Artifact replaces its entire sequence,
including multiple Tj/TJ/quote operations. Unicode and empty strings are retained;
empty replacement suppresses source glyph text. Replacements are concatenated
without invented spaces, word breaks or layout. A graphical/empty sequence may
carry replacement text even without a glyph source.

The outermost replacement is an indivisible logical unit: nested replacement
strings do not also appear, and its characters are not partitioned according to
inner markers. Artifact filtering removes an artifact sequence and its descendants;
an outer non-artifact replacement remains one unit rather than guessing which
substring corresponds to an inner artifact. Disabling replacement exposes the
underlying glyph mappings, with artifact filtering still controlled separately.
Lang/Alt metadata is checked as text strings but is not replacement text; language
inheritance and Alt-description synthesis are not performed.

## Sources and unknown text

`MarkedTextReport` contains a DocumentStamp, page handle, logical spans and
diagnostics. `MarkedTextSpan` contains optional text, an optional
`ActualTextSource`, covered text-show locations and innermost structural content
references. Locations identify the page, Form invocation chain, operation index
and decoded operator byte offset. They are not file offsets or individual glyph
edit addresses. A page Contents array is a concatenated execution stream; an
operation or sequence may span its chunks in the supported PDF 1.x model.

Replacement text has no fabricated character code, advance or bounding box.
Use existing character APIs for original glyph geometry and byte ranges; do not
feed replacement text into an editor as if it were encoded source glyphs.
Missing Unicode stays `None`. `report.text()` refuses unmapped glyphs and content
diagnostics instead of returning a silently repaired string. Spans and diagnostics
remain available for inspection. Requery after edits or reopen; object identities
belong to one open document.

## Declared structure order

`extract_text_structure_order()` first validates the supported hierarchy,
namespace/attribute model and bidirectional ParentTree associations. It then
returns logical units in declared depth-first K order, not paint order or inferred
layout. An absent tree returns `None`; an empty declared tree remains distinct.
Structure-element ActualText replaces its subtree once, including descendant
marked replacements, with the element handle as its explicit replacement source.
The same option disables both element and marked-content replacements.

Content is assigned by page/Form identity and MCID. An atomic marked replacement
covering several MCIDs of one owner is emitted once at its first K position; one
crossing different owners rejects. A structure MCID in a shared Form must have a
unique physical invocation: multiple instances or a mismatching declared page
reject, rather than duplicating text or selecting an instance. Page extraction
still supports repeated Form calls with separate source chains.

Unassociated paint-order units are counted in `unassociated_spans`, not inserted
into a guessed reading order. OBJR without an enclosing element replacement is
counted in `unextracted_object_references`; no annotation appearance, whole-XObject
text or Alt description is synthesized. PDF 2.0 Artifact elements are filtered
only when the qualified role resolves to Artifact in `http://iso.org/pdf2/ssn`;
an identically named custom role is not guessed to be an artifact.

## Limits and scope

Interpretation uses the existing supported font/graphics model, including its
validation and limits; ActualText does not bypass an unsupported source format.
Operations, decoded invocation bytes, captured Unicode, semantic entries and
source/metadata work are bounded across nested Forms. Structure extraction shares
those work counters across pages, including filtered artifacts and overridden
replacement strings. Source retention, cached structure units and replacement
aggregation have additional cumulative limits. These are logical work limits,
not a complete shared allocation/RSS guarantee.

Unsupported properties reject explicitly. The original extraction entry points
still refuse OC marked content and XObject OC; the new
[optional-content entry points](optional-content.md) accept an explicit visibility
snapshot for filtering instead of silently changing legacy extraction.
Appearance-stream MCR/StmOwn,
IDTree, arbitrary schemas, full Tagged PDF/PDF-UA conformance, language/layout
inheritance, bidi, OCR and heuristic paragraph/column reconstruction remain out
of scope. Supported [structure-aware transfers and body edits](structure-transfers.md)
use separate validated transactions; unsupported graph changes still reject.

## Verification

```sh
cargo test --locked -j2 -p pdf --test v010_marked_text --test v03_marked_content
cargo build --locked -j2 -p pdf --example extract_marked_text
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 \
  docs/check-v010-marked-text.py <new-output-directory>
```

The independent gate uses existing pikepdf, qpdf and MuPDF. It compares declared
logical strings and K order, checks strict rewrite and raw object/content retention,
observes MuPDF's marked ActualText extraction, and rejects semantic counterexamples.
It does not equate layout ordering, rendering or PDF/UA conformance across engines.
No Poppler, dependency installation or fuzzing is needed.

Basis: [ISO 32000-1:2008, 14.6–14.9](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)
and [PDF 2.0 chapter 14 errata](https://pdf-issues.pdfa.org/32000-2-2020/clause14.html).
