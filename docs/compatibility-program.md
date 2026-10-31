# Real-document compatibility and bounded rendering

This program measures practical reading, rendering, text, full-save reopening and
editing independently. Text success records API completion and missing Unicode
counts, not proof that every character has the correct semantic mapping. It does not claim support for every possible PDF or use
historical version acceptance as evidence for the current build. OCR, inferred
tables/layout, fuzzing and Poppler are outside this work.

## Corpus and evidence

The initial local inventory contains 108 unique real documents and 38,074 pages,
including two previously downloaded public benchmark documents. Seven files
exceed 64 MiB. Private paths, metadata, PDFs and page images stay under ignored
`output/compatibility-20261031/`. This is a convenience sample of available files,
not a statistically representative survey of all PDFs. Report both document and
page counts; a long scanned book must not masquerade as thousands of independent
document successes. Synthetic regression fixtures are a separate cohort.

```sh
cargo build --locked -p render --example compatibility_probe
output/benchmark-venv/bin/python scripts/inventory_pdf_corpus.py \
  ~/Documents/paper ~/Downloads output/benchmark-real-20261027 \
  --output output/compatibility-20261031/manifest.json
output/benchmark-venv/bin/python scripts/compatibility_corpus.py \
  output/compatibility-20261031/manifest.json \
  --output output/compatibility-20261031/native --timeout 300 --jobs 2
```

The native probe uses file-backed input and browser policy values. It is explicitly
**not actual Wasm evidence**. It uses `budgeted` image decoding; the browser
runner uses explicit target previews. Those policy differences prevent a causal
before/after quality or speed claim across native and browser runs. Each document runs in an isolated process with a
deadline; progress is flushed as JSONL. Every source hash is checked before use.
Killed/timed-out documents retain their completed events; remaining pages count
as untested, never successful. The runner separates render failures, text failures
and MuPDF overview review signals. `--require-clean` makes any failure, untested
page or review signal fail the gate. Keep the probe binary/hash with the report.

The actual browser runner tests every page through the optimized module, Worker
and `openBlob(File)`. It records open/render/text/save-reopen/rotation-roundtrip
separately and never rewrites the source file. Per-operation Worker timeouts are
30 seconds. MuPDF and PDFium independently screen bounded page overviews; review
images are retained only when dimensions, blankness, ink coverage or pixel error
trigger review. These thresholds flag candidates; they are not visual correctness
proofs. In particular, small missing features require targeted high-DPI oracles.
Browser event milliseconds include reference screening and bridge I/O; they are
not engine-only rendering benchmarks.

```sh
export RZPDF_COMPAT_MANIFEST="$PWD/output/compatibility-20261031/manifest.json"
export RZPDF_COMPAT_OUTPUT="$PWD/output/compatibility-20261031/browser"
export RZPDF_CHROMIUM="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
python3 scripts/run_browser_compatibility.py "$RZPDF_COMPAT_MANIFEST" \
  --output "$RZPDF_COMPAT_OUTPUT" --jobs 4
```

Use `--web` to select a frozen copy containing `dist/`. The coordinator runs
independent document processes and merges only matching input, driver, module,
SDK and reference-engine hashes. Missing/duplicate outcomes and mixed builds
cannot produce a successful aggregate. The single-process driver remains usable
directly with Node; its shard parameters are internal coordinator inputs.
`--merge-only` recomputes denominators from existing immutable shard evidence;
unknown exit status or missing results remain incomplete. `readableDocuments`
(and its clearer alias `renderCompleteDocuments`) counts successful render API
completion for every page; it is not a visual-correctness certificate. Visual
review/reference failures and unknown Unicode mappings have separate counters.
The edit probe checks rotation roundtrip geometry in memory. The save probe
checks full-save output, strict reopening and page count, not every saved page
rendered again or persistence of every possible edit kind.

Set `RZPDF_COMPAT_REQUIRE_CLEAN=1` for a strict private-corpus gate. Reports pin
input, font-policy and module/SDK hashes and preserve failed pages. Missing permissions/fonts,
unsupported features, resource limits, malformed input, traps and untested pages
must remain visible; none is converted to a success by an allowlist. Broad results
are local evidence. Public CI runs only redistributable synthetic fixtures.

The default corpus run does not discover host fonts or register application
substitutes. A missing-substitute error is therefore a font dependency result,
not evidence that PDF syntax or an image decoder failed. A separate diagnostic
cohort can explicitly register a licensed local font before opening:

```sh
RZPDF_COMPAT_SUBSTITUTE_FONT="$PWD/font-data/fonts/n019003l.pfb" \
  python3 scripts/run_browser_compatibility.py PRIVATE_MANIFEST \
  --output PRIVATE_OUTPUT --jobs 4
```

This single-font fallback retains PDF positioning but does not emulate a full
family/weight/language matcher or certify typography. Production applications
should supply the matching licensed faces through the font service. Reports pin
the supplied bytes; the merger rejects mixed font policies. Font-dependent
cohorts are diagnostic selections, not extra representative-population evidence.

## Resource model implemented in this slice

| Resource | Scope | Browser default / conservative |
|---|---|---|
| `maxImageSourceBytes` | One streaming source; includes PNG predictor tags | 128 / 64 MiB |
| `maxImageDecodeWork` | Conservative source-sample/mask-grid units per preparation | 536870912 / 268435456 units |
| `maxDecodedBytes` | One decoded stream or target image RGBA | 16 / 8 MiB |
| `maxOperationBytes` | Simultaneous explicitly leased operation owners | 64 / 32 MiB |
| `maxDisplayListBytes` | Retained interpreted payload | 32 / 16 MiB |
| `maxRenderBytes` | Raster allocation path, including simultaneously live group surfaces | 16 / 8 MB |

Flate traverses source rows even when the target is small. Cumulative traversal
must not be confused with simultaneous memory: an 8.75 MP RGB image requires
26,250,000 source sample bytes, but its 700 × 980 target/rows/accumulators occupy
less than 3 MiB in the tracked decoder pool. Preview images in the same nested
preparation share the pool and work meter. Cached decoded images do not consume
decode work again. Scope guards restore accounting after errors/unwinding.

The display collector charges shared ICC profile, pattern, mask and shading
owners once per list, rather than once per draw. Pending owner identities are
published only after a draw passes its checks. Cloned path/clip geometry remains
separately charged; sharing a profile does not exempt independently owned paths.
Display-list exhaustion reports `display_list_bytes` at `page_prepare` with
`limit` and `actual`.

This is explicit-owner accounting, **not a global allocator or RSS ceiling**.
Legacy decoders, caches, Skia, fixed codec state and JS still have distinct bounds.
Further allocator/lease migration remains tracked by AR-03; do not close that
parent item based on this slice. The unchanged 256 MiB Wasm maximum is capacity,
not a guarantee that an arbitrary next allocation will succeed.

## Common combinations addressed

- Flate RGB/Gray target previews use the source-work ceiling instead of rejecting
  cumulative inflation above the resident limit. Their previous unconditional
  16 MP source ceiling is removed; source bytes, rows, output and operation work
  remain independently bounded. Corrupt checksums never publish previews.
- Sequential/progressive JPEG use reduced IDCT. Progressive source-sized padded
  MCU coefficient storage has a conservative scratch reservation before starting
  decompression. Source work is byte-bounded; reduced output is not proof of a
  small progressive decoder allocation.
- CCITT Gray/stencil target previews sample decoded black/white runs directly,
  including fractional stencil coverage and Decode/BlackIs1 semantics. The path
  checks source work, target output, transition scratch and exact row completion;
  it never builds a full-source byte-per-pixel image. Gray/stencil only; indexed
  palettes, filter chains and other masks retain the bounded legacy path.
- JBIG2 Gray/stencil target previews use area coverage after a packed 1-bit
  decode, retaining fine strokes and fractional stencil opacity. The native
  allocator shares the operation pool with source bytes and target storage;
  preflight segment work is charged before decoding. This is packed-source plus
  target storage, not native reduced-resolution/ROI decoding. Shared Globals,
  exact header dimensions and existing segment restrictions remain enforced.
- FreeType text retries without hinting when hinted bounds are empty while
  unhinted bounds contain visible geometry. This preserves legitimate empty
  glyphs and avoids silently dropping valid outlines after bytecode failure.
  Scratch is fixed-size; CoreText/DirectWrite keep their existing policy.
  A real page with missing body paragraphs now passes both reference screenings;
  redistributable geometric-font fixtures check visible ink at 18/36/72 DPI
  through both browser transports.
- Unembedded explicitly nonsymbolic simple fonts use StandardEncoding when the
  Encoding dictionary omits BaseEncoding, preserving Differences. This follows
  PDF 1.7 table 5.11 and pp. 430–431, rather than inventing CID/GID mappings.
  Embedded and symbolic fonts retain their actual program/encoding semantics.
  [Adobe PDF reference](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.7old.pdf).
- Compatible mode permits the same diagnosed one/two-byte EOL surplus for stencil
  images as ordinary images; Strict still rejects it. Missing samples or arbitrary
  extra bytes remain errors.

Complex ICC/CMYK/Indexed streaming conversion, JPEG-plus-mask combinations,
JBIG2 with additional masks, JPX reduction with proven scratch bounds, remaining Type3
transparency combinations and ambiguous missing-font mappings are not silently
approximated. JPX nonzero-origin/multitile and environment-dependent optional
content usage rules also retain their existing explicit refusals. The corpus reports determine the next
specific fixes. Supporting one codec is not supporting every codec/mask/color
combination.

## Browser source and rendering APIs

`WorkerPdfEngine.openBlob(file, options)` uses an immutable Blob/File backing and
at most 64 KiB reads, up to 2 GiB source length. The parser keeps its existing
Read/Seek interface. Each source ID has one Rust reader owner; failed opening,
document close and module disposal release the JS reference. JS exceptions become
I/O errors at the bridge. Large unknown streams are not loaded just to open a page.
Byte-array `open()` remains available with its shared-buffer constraints.
Password options and effective profiles use the same native option parser.
Full-save output remains bounded; reader input does not enable incremental
signature append, which requires the immutable byte-source baseline.

`renderTiles` streams bounded regions at the caller's DPI; `renderPreview` selects
an explicit fitted DPI and assembles only a bounded overview. Both adapters use
the same planner. They check document revision between tiles and do not retry a
source-image failure with ineffective smaller tiles. Preview defaults to target
decoding; existing render() semantics are unchanged. Raster headroom is a planning
heuristic, not a guarantee that every nested group fits.

Engine statistics expose linear memory **capacity** and external Blob source
counts/bytes separately. Releasing objects is distinct from shrinking Wasm memory
back to the operating system.

## Repeatable acceptance

1. Run native image/resource/font tests and renderer/ABI regressions for changed
   paths; run fmt, clippy and SDK build/protocol/planner checks.
2. Rebuild optimized Wasm from the final source inventory. Preserve artifact hashes.
3. Run `compatibility-resources-browser.mjs` in sync/Worker, plus existing image,
   mask, source-stream, Unicode and transparency-group browser gates. The new gate
   covers 8.75 MP scans, exact color/alpha oracles and an 80 MiB sparse local source
   whose unused stream must remain lazy, failed open/close and independent owners.
4. Run the private real corpus separately. Publish measured counts and unresolved
   groups; do not mark compatibility complete merely because synthetic tests pass.
5. Compare reports with `scripts/compare_compatibility_reports.py`. Preserve input
   identity, bounds and environment differences; performance numbers from unlike
   native/debug/browser builds are not comparable.

## 2026-10-31 measured checkpoint

These runs are separate cohorts. Do not add them together as independent samples
or apply a newer targeted result to an older full-corpus denominator.

| Cohort | Documents / pages | Render passed / failed / untested | Visual review pages |
|---|---:|---:|---:|
| Broad default-font run, frozen pre-JBIG2 module | 108 / 38,074 | 30,881 / 5,207 / 1,986 | 394 |
| Explicit single-substitute diagnostic cohort, pre-JBIG2 | 12 / 7,242 | 7,194 / 48 / 0 | 361 |
| Final-module JBIG2 diagnostic cohort, explicit substitute | 4 / 2,522 | 2,482 / 40 / 0 | 3 |
| Final-module specified real files | 4 / 197 | 197 / 0 / 0 | 0 |

The broad run opened 106 documents and rejected two. All-page render API completion
was 66 documents; 63 had no overview screening signal. Text API results were
34,759 passed, 1,329 failed and 1,986 untested, with 2,590,563 unmapped Unicode
characters on 4,659 successfully returned pages. Full-save strict reopen/page-count
checks passed 65, failed 39 and left four untested; in-memory rotation roundtrips
passed 95, failed seven and left six untested. The report is **not clean**.
Unopened files and remaining pages after two 30-second render timeouts account
for the untested render pages; two separate save timeouts also expired sessions.
Eight concurrent shards and reference rendering affect deadlines; a timeout is
not proof of a format refusal or a Wasm trap.

The broad render failures comprise 2,767 font errors, 1,380 unsupported-feature
errors, 763 resource errors and 297 format errors. Dominant measured groups:

| Boundary | Observed failed pages | Handling / next work |
|---|---:|---|
| Missing substitutes or undrawable font glyphs | 2,767 | Supply licensed matching faces through the existing font service; diagnose invalid embedded glyph mappings separately. A single generic fallback is not typography acceptance. |
| Optional-content usage category needs an environment | 1,003 | Add explicit view/zoom environment semantics; do not silently display every layer. |
| Missing optional-content default configuration | 287 | Diagnose compatible interpretation with independent references. |
| JBIG2 source-pixel gate, ordinary Gray/stencil | 691 | Implemented packed-source area preview; tested separately on the final module, not retroactively counted in the broad run. |
| JPX nonzero origins or multiple tiles | 354 | Implement decoder layouts with bounded workspace. |
| JPX with explicit JBIG2 masks | 37 | Add aligned color/mask reduction; ordinary JBIG2 preview does not implement this combination. |
| JPEG plus masks | 10 | Generalize coupled sampling with Decode/Matte and independent source grids. |
| JPEG parameter layouts | 6 | Validate supported DecodeParms normalization before entering reduced-IDCT decoding. |
| Complex Flate color spaces / ICC masks | 2 | Add bounded color conversion instead of treating them as DeviceRGB. |

These counts describe that frozen run, not universal feature frequency. Remaining
resource failures include path/content budgets, display payloads and ICC workspace.
The open failures were an xref-count resource refusal and unsupported V2/R3
security. Save refusals also include guarded linearization rewrites, signature
protection, output budgets, explicit version-upgrade requirements and malformed
xref/object recovery. A successful read does not promise permission or resources
to save, and these refusals must not be bypassed to increase a pass count.

The final specified-files cohort passed rendering, text API calls, full-save
strict reopening and rotation roundtrip for all four documents. Both references
screened all 197 pages without review flags; 16 unmapped Unicode characters remain
on four pages. Text semantics therefore still need application-level checking.

The final JBIG2 diagnostic cohort tested every page, with no untested operations.
690 pages previously reporting an ordinary JBIG2 source-pixel refusal returned
successful renders in this cohort. The one remaining source-pixel refusal was
verified separately on selected page 198 of a fifth document, also passing both
reference screenings. Thus all 691 originally affected pages have successful
render results; the other pages of that fifth document were not rerun. This is a matched-page observation; the font
policy differs from the broad run, so it is not a same-policy whole-corpus score.
The 40 remaining render failures are 36 JPX images with explicit JBIG2 masks,
three ICC header/signature errors and one missing CJK substitute. The three
screening signals are nearly blank pages with tiny text near the ink threshold;
they remain recorded as review candidates rather than waived. Only one of these
four documents completed every operation without a refusal. Save/reopen passed
one and failed three (two guarded linearization rewrites and one malformed object
stream); rotation roundtrip passed three and failed one on that malformed source.
Text calls passed 2,519 and failed three, with 51 unmapped characters on one page.
This diagnostic selection is evidence for the changed path, not a general PDF
compatibility percentage. Its reports are in `jbig2-cohort-final` and matched
page transitions in `jbig2-paired-pages.json`.

For the minified scanned table that exposed lost JBIG2 strokes, the same 348×512
page compared with PDFium at a 256×256 metric grid changed from normalized MAE
0.042206 to 0.006214; thresholded ink coverage changed from 44.31% to 99.96% of
the reference. Pixel images were also inspected. This single selected page is a
quality regression oracle, not an all-document score. The default full-resolution
`budgeted` policy is unchanged; explicit target previews receive area reduction.

Evidence remains local under `output/compatibility-20261031/`: `final-browser`,
`font-cohort-final`, `known-cases-bilevel-final`, `resources-teardown-final`,
`regressions-bilevel`, `thin-stroke-page47`, `jbig2-page198` and
`wasm-build-bilevel.json`.
The broad module SHA-256 starts `07cc4210`; final runtime SHA-256 is
`cbc39b012d3f6c675d26e81f06704bbad08fbcf4d4a0540ee2ee37202e7a1629`.
Each report carries the full runtime, SDK, input and driver hashes. Earlier driver
and browser-helper snapshots are preserved alongside the private reports. The
final helper additionally destroys only the launcher's own stdio stream ends,
so detached Chrome crash collectors cannot keep a completed Node gate alive;
it does not search for or stop system updater processes.

The native regression campaign covers 879 core tests, 871 integration tests,
118 renderer tests, 27 ABI tests, six PDF doctests and one font-data test: 1,902
passing checks and one pre-existing ignored test. Integration coverage comprises
the broad 96-target run plus final changed-codec reruns, not a claim that every
integration target was rerun after the last codec change. Workspace all-target
Clippy with warnings denied passed; the pinned optimized Wasm build, SDK tests,
11 aggregation tests and packaging checks passed. The final actual-Wasm gates
cover sync/Worker source, image, masked-image, Unicode and transparency paths,
plus Blob R4/R6 passwords/profiles, exact pixel parity, reference ownership,
80 MiB lazy input, unsigned source IDs and exhaustion cleanup. Remote CI has not
been run from this workspace.
