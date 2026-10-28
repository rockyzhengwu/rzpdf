# Explicit-font Unicode FreeText

Implemented for pdf-editor RZPDF 001 (`specs/validation/rzpdf-requests.md`, baseline
21fa918, 2026-10-28). This is a bounded horizontal-text appearance writer, not a
paragraph layout or shaping engine. [Validation](#validation) records the native,
Wasm/SDK/Worker and independent-reader evidence.

## Native API

`document::annotations` exports `UnicodeFreeTextFont` and `UnicodeFreeTextBinding`.
Use ordinary `AnnotationKind::FreeText { font_size, alignment }` data with:

| Method | Font source | Result |
|---|---|---|
| `add_unicode_freetext(page, annotation, font)` | Explicit static TrueType bytes and embedding metadata | Document-local annotation handle |
| `update_unicode_freetext(page, handle, annotation, font)` | `None` reuses the current repertoire; `Some` supplies a replacement font asset | Same annotation identity, new isolated appearance |
| `*_with_budget(..., budget)` | Same contract, explicit shared operation pool | Transaction and prepared-font/output work intersect the pool |
| `unicode_freetext_binding(handle)` | Reads a persistent binding | Embedded font handle and supported Unicode repertoire, or `None` |

The repertoire is the distinct Unicode scalars of the initial Contents, excluding
LF (an empty initial string seeds U+0020, which the font must cover). Encoding is fixed Identity-H with CIDToGIDMap/ToUnicode; horizontal advances,
line metrics and outlines come from the supplied SFNT data. This declares the
mapping/metric source independently of caller-supplied FontDescriptor metadata.

The [runnable example](../pdf/examples/unicode_freetext.rs) creates all four page
rotations with three alignments, saves, strictly reopens and edits without a new
asset. It uses the existing [OFL-licensed static fixture](../pdf/tests/fixtures/v03/README.md),
not a system font. `UnicodeFreeTextFont` has these fields:

- `bytes`: borrowed static single-face glyf TrueType input for this operation.
- `embedding`: existing `TrueTypeEmbeddingOptions`; PostScript identity and
  caller-supplied `cap_height`/`stem_v` in PDF's 1000-unit font space, plus `serif`.
- `omit_layout_and_auxiliary`: explicit opt-in to the existing subset table
  omission policy. The font must pass OS/2 editable-embedding and subsetting
  checks. Display registration is never treated as embedding authority.

No source byte buffer is retained by these APIs. The document owns the produced
font subset, CIDToGIDMap, widths, ToUnicode and AP streams. The document store,
source caches and native font allocations retain the engine's existing accounting
exclusions: the operation budget is not a process RSS ceiling.

## SDK and Worker

Both transports use the existing `apply` mechanism, with two new operations:
`addUnicodeFreeText` and `updateUnicodeFreeText`. These do not change the ABI or
Worker envelope version. A creation request requires `font: FontWriteOptions`
and a binary font asset. An update accepts neither, or both; never one alone.

```typescript
import {createWorkerEngine} from './dist/worker-client.js';

const engine = await createWorkerEngine();
const doc = await engine.open(pdfBytes, {mode: 'strict'});
const font = {
  postscriptName: 'NotoSansSC-Regular', capHeight: 733, stemV: 80,
  serif: false, omitLayoutAndAuxiliary: true,
};
// fontBytes must be an explicitly selected, embeddable static TrueType font
// covering the requested text; these metadata values belong to this example.
try {
  const created = await doc.apply({
    operation: 'addUnicodeFreeText', stamp: (await doc.state()).stamp, page: 0,
    annotation: {
      kind: 'FreeText', rect: [20, 20, 300, 100],
      contents: '你好，世界！Hello PDF\n你好',
      fontSize: 16, alignment: 'left', color: [0, 0, 0],
    }, font,
  }, {asset: fontBytes});

  const snapshot = await doc.annotations(0);
  const record = snapshot.records.find(r =>
    r.object?.[0] === created.created?.[0] && r.unicodeFreeText);
  if (!record?.object || !record.annotation) throw new Error('Missing annotation');
  await doc.apply({
    operation: 'updateUnicodeFreeText', stamp: snapshot.stamp, page: 0,
    object: record.object,
    annotation: {...record.annotation, kind: 'FreeText',
      contents: 'Hello PDF\n世界', fontSize: 14, alignment: 'right'},
  }); // No asset: every character is already in the immutable repertoire.
  const saved = await doc.save();
  // Strictly reopen saved.bytes, obtain a NEW annotation snapshot and continue.
} finally {
  await doc.close(); engine.dispose();
}
```

For the direct SDK, use `createEngine()` and pass bytes as the second argument:
`doc.apply(request, fontBytes)`. The Worker form is `{asset: fontBytes}`. Query
records add `unicodeFreeText: {fontObject, repertoire} | null`; `fontObject` is
read-only document-local information, not a display-font ID or a cross-document
write token. Handle/object identity must be combined with a fresh snapshot stamp.

Adding characters outside that repertoire requires `font` and `asset` again on
`updateUnicodeFreeText`. This explicitly replaces the font/AP binding. The original
full font cannot be recovered from an embedded subset. Applications that want to
add future characters must retain/reacquire the original asset or let the user
select another supported font. Changing only size/alignment or using the current
repertoire needs no original asset, including after saving/reopening.

## Supported layout and output

- Mixed Chinese/Latin with explicit LF, positive finite font size and left/center/
  right alignment. Independent Latin scalars U+0020–007E and U+00A0–02FF,
  CJK punctuation/kana U+3000–30FF (excluding combining marks U+3099/309A),
  Han U+3400–9FFF / U+20000–323AF, and fullwidth U+FF01–FF60 are accepted when
  the supplied font has a valid glyph. Other scripts/sequences explicitly refuse
  in this first layout profile. There is no normalization, font fallback, kerning,
  bidi, ligature formation, vertical writing or complex shaping.
- CR, TAB, combining/variation sequences and automatic wrapping are not accepted.
  Normalize application line endings to LF before submission.
- Coordinates and font size use default unrotated PDF user-space units; UserUnit
  scales physical output. The writer preserves page rotation, CropBox and UserUnit.
  Rectangles are `[left, bottom, right, top]`, not browser display coordinates.
- Two user units of padding; line leading is the maximum of font ascent/descent
  and actual glyph bounds, plus 0.2 em. Placement uses real advances and tight
  unhinted simple/composite outline bounds. Overhangs count toward fitting.
  No text is silently shrunk, dropped or clipped to force a fit.
- Self-contained normal AP resources contain F0; DA includes its font/size/RGB,
  Q stores alignment, Contents stores Unicode, and a versioned `RzFreeText`
  binding preserves the immutable repertoire. Opacity wrappers also expose the
  DA font in the outer AP scope. Independent readers need not understand the
  private binding to display or extract the appearance text.
- Reuse validates actual font Unicode/GID/width mappings against the binding.
  Actual embedding rights and static-format checks are repeated after reopening.
  AP/font mismatches and unknown binding versions refuse rather than rewriting
  an arbitrary external FreeText appearance. Page copies can share immutable
  fonts; an edit creates a new AP and, when requested, a new font subset.
- Existing `addAnnotation`/`updateAnnotation` retain the ASCII Courier profile.
  Legacy update refuses an explicitly bound Unicode appearance; use the new
  update operation. A legacy supported FreeText can be upgraded by supplying
  an explicit font to the new update operation.

Font assets intersect the 32 MiB transport cap, active font/raw/decoded limits and
shared operation budget. Wasm Contents input remains limited to 16 KiB of UTF-8,
within the existing 64 KiB mutation JSON envelope; annotation query metadata is
bounded to 8 MiB. Those are limits, not a promise that every input under one cap
fits the other budgets. Font errors cover missing/repertoire-excluded glyphs and
invalid fonts; unsupported layout/format and embedding restrictions return
`unsupported`; invalid geometry/fit
uses the existing `format` classification; budget failures use `resource_limit`.
All are ordinary nonfatal errors. Refresh snapshots before retrying: failed
transactions may invalidate stamps, but commit revision, annotations and pixels
must remain unchanged. Save failure is read-only.

Unencrypted and authenticated R6 annotation workflows are verified. Annotation
permission alone is sufficient even without content-modification permission;
the internal permission scope cannot escape to later content writes and does
not bypass signed-source guards. R4 save and general signed-source editing retain
their existing unsupported boundaries. Full rewrite retains the existing
object-preservation policy: replacing text/AP references is not secure erasure.

## Validation

2026-10-28–29, macOS x86_64. All final reports are under the ignored local directory
`output/unicode-freetext-20261028/`; scripts and licensed inputs are committed.

- Seven native FreeText tests plus 54 related annotation/form/security/save tests:
  **61 passed**. Fifteen context loading/rollback tests passed separately.
- Web/Wasm host interface tests: **25 passed**, including a new binary-asset,
  strict reopen, no-asset update, failed-repertoire rollback and render test.
- Strict Clippy for both libraries, the new native test/example; format,
  TypeScript build and deterministic SDK/Worker protocol checks passed.
- Pinned optimized Wasm build and Chrome 156.0.8078.48, direct SDK and actual
  Worker: creation, all alignments, explicit lines, saved/reopened updates,
  fresh-font repertoire expansion, unchanged neighboring pages, real shared
  font page-copy isolation, bad font/missing glyph/overflow/save-limit rollback,
  R6 annotate-only permission, signed-source refusal, old ASCII roundtrip and
  zero tracked resources after close all passed. Only hashes and saved PDF bytes
  cross the test bridge; the RGBA arrays stay in the browser.
- MuPDF 1.26.7 and PDFium: **24 native + 56 browser AP text/alignment checks**;
  **16 native + 52 browser page/DPI pairs** at 72/144 DPI passed. qpdf checks all
  eight output files. MuPDF extracts real appearance text matching Contents,
  including Chinese, and both readers render nonblank matching artwork. Maximum
  normalized cross-reader MAE was 0.004164, below the fixed 0.025 gate. This is
  bounded agreement, not identical antialiasing between different readers.
- Standalone PNGs were inspected for native Chinese/alignment, browser updated
  glyphs and combined 270-degree rotation/CropBox/UserUnit output.

The verifier explicitly applies UserUnit to PDFium's default-unit page render
helper. It normalizes MuPDF text coordinates using actual rotated viewport sizes;
PyMuPDF 1.26's annotation derotation translation does not scale with UserUnit.
These adapter adjustments leave PDF attributes and acceptance thresholds intact.

Reproduce with the existing toolchain (no downloads/Poppler/fuzz required):

```sh
cargo test --locked -p pdf --test unicode_freetext --test v09_annotations --test v09_forms --test v05_security --test v012_layout_security --test v011_controlled_updates --test linearized_save
cargo test --locked -p pdf --lib pdf_context::loading_tests
cargo test --locked -p web-wasm --lib
cargo clippy --locked -p pdf -p web-wasm --lib --test unicode_freetext --example unicode_freetext -- -D warnings
cargo fmt --all -- --check
cargo run --locked -p pdf --example unicode_freetext -- output/freetext-fixtures-NEW
python3 scripts/build_webassembly.py --package web-wasm --output output/freetext-wasm-NEW.json
npm --prefix web run build
node web/tests/unicode-freetext-protocol.mjs
# Set RZPDF_CHROMIUM, RZPDF_FREETEXT_FIXTURES and RZPDF_FREETEXT_OUTPUT as needed:
node web/tests/unicode-freetext-browser.mjs
python3 docs/check-unicode-freetext.py output/freetext-fixtures-NEW output/freetext-native-check-NEW
python3 docs/check-unicode-freetext.py output/webassembly/unicode-freetext output/freetext-web-check-NEW
```

The browser run used the repository-pinned Node 22.15.1 with the dynamic-import
launcher described in [build instructions](webassembly/build.md); pinned local
paths and Chrome are environment-specific. The evidence does not assert a full
workspace run, all font formats/scripts, arbitrary external AP editing, other
platforms or editor-application acceptance.

## Editor handoff

Pin the implementation commit, rebuild the native and Web artifacts, and add a
positive explicit-font probe before marking RZPDF 001 supported in the editor.
The original probe uses **legacy** `addAnnotation`, so Chinese must still refuse
there. Rename that check to describe the legacy profile and preserve its
failure-atomicity assertion; add a new `addUnicodeFreeText` asset-based positive
probe, strict save/reopen/update and independent-reader checks. Do not simply
flip the old request's expected boolean.

Use fresh annotation snapshots after reopen/failures. Retain font assets at the
application layer for future new characters. The editor owns input methods,
text-box interaction and input layout; this writer owns the persisted PDF glyph
mapping and normal appearance. OCR, rich text, paragraph layout, automatic font
selection, user undo history and cooperative cancellation remain separate work.
