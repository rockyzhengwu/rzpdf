# Text replay attribution and exact glyph-run batching

This round refreshes the complete native three-engine comparison and adds a
pixel-preserving optimization for ordinary filled text. Image decode policy,
font selection, extraction, DPI and resource limits retain their prior behavior.

## Attribution

An opt-in `render-diagnostics` Cargo feature records inclusive replay phases:
text, font-manager acquisition, typeface loading, glyph selection, glyph paint,
batched glyph paint, prepared paths and canonical coverage rasterization.
Normal builds compile out the recorder, fields and timing calls. The benchmark
worker rejects `profile_render: true` unless the feature is enabled. It snapshots
first-replay timings and disables recording before warm replays. Diagnostic
builds are not used for engine rankings.

Baseline 144 DPI native attribution, medians from three fresh processes:

| Document | Replay (ms) | Manager (ms) | Typeface load (ms) | Glyph selection (ms) | Glyph paint (ms) |
|---|---:|---:|---:|---:|---:|
| Real PLOS paper | 48.32 | 8.77 | 1.44 | 0.33 | 19.29 |
| Dense Latin | 22.01 | 8.22 | 2.00 | 0.27 | 5.17 |
| Embedded TrueType | 19.56 | 8.45 | 1.13 | 0.07 | 3.44 |

The curve-only page spends 78.87 ms in prepared paths out of 84.62 ms replay;
canonical coverage rasterization accounts for 31.37 ms within that path time.
These phases overlap and are not complete exclusive accounting. A separate
macOS `sample` capture of repeated paper replay also shows substantial work
beneath per-glyph `SkCanvas::drawGlyphs` and glyph-run/strike construction.

The real paper often declares font size 1 and puts the visible size in its
text matrix (for example 19.9253). Restricting batches to power-of-two transforms
would miss this file. The final diagnostic probe observes 75 batched runs
among its 118 text paints. Source sampling and diagnostic probes guide the
change; the serial default-build comparisons below establish its performance.

## Exact-origin fast path

Previously, each filled glyph independently snapped its device origin to a
1/256-pixel grid, set a canvas matrix and called Skia with a single glyph.
The new `glyph_run` module submits eligible runs with one shared matrix.
It retains the same glyph IDs, font, paint order and independently snapped
origins; it does not replace PDF positions with shaping or font advances.

Eligibility is deliberately bounded:

- Axis-aligned finite, nonzero scales; opaque solid SrcOver fills without a
  shader. Stroke/fill-stroke, translucent, shader and complex transforms retain
  the existing glyph path.
- Effective glyph scale stays within the existing 256-pixel bitmap cache cap;
  large vector glyphs retain canonical coverage rasterization.
- Binary scales use exact coordinate division. General diagonal scales anchor
  the matrix at the first snapped glyph and check every reconstructed device
  coordinate under both fused multiply-add and separate arithmetic. Adjacent
  float candidates address division rounding. If either model cannot reproduce
  an origin exactly, the entire run falls back before painting.
- Coordinates stay in the precision range of the existing 1/256 grid. Optional
  scratch allocation covers both position buffers and glyph IDs within the
  existing operation allowance; failure simply uses the original path.

Default knockout handling still splits members as before. No cache budgets,
font retention limits or global font initialization were increased. The
CoreText UI-font anchor remains unchanged and is still a cold-start cost.

## Verification

- Render library: 115 tests passed; WASM interface: 24 tests passed.
- New direct tests compare batched output to independent per-glyph drawing at
  fractional origins, overlapping positions, negative scales and nonbinary
  scales. Unsupported transforms, blend/alpha and large glyph cases fall back
  without painting. Existing text-prefix, empty .notdef, vertical text,
  clipping, groups, masks, patterns and fractional-DPI tile tests also pass.
- All 44 synthetic native page outputs and all 12 real native page outputs
  remain byte-identical to the frozen previous worker.
- Pinned release WASM: 56 synthetic and eight real browser combinations pass.
  Four frozen-module text cases (paper, dense Latin, TrueType, multi-page CJK)
  also match complete RGBA SHA-256 and dimensions for seven region requests
  each: 28 comparisons spanning 72/96/144 DPI, moved regions and page changes.
- ICC/Type3 and resource-profile browser checks pass in both transports.
  Six Python benchmark harness tests also pass.
- Default and diagnostic-feature Clippy pass with Rust warnings denied.
  Existing macOS mozjpeg assembly linker messages are unchanged.

The frozen-browser helper now hashes the complete owned RGBA buffer inside
Chrome instead of exporting every channel as a JSON number. Dimensions, byte
length and SHA-256 are compared and retained in the report. This removes large
JSON-array transfers and expensive Node comparisons; it does not relax image
thresholds or change engine output. Browser timing is not used to rank engines
in this report.

## Reproduction and artifacts

```sh
cargo build --locked --release -p render --example benchmark_render
python3.11 scripts/benchmark_pdf.py \
  --manifest output/benchmark-corpus-v2/manifest.json \
  --output output/text-after --dpi 72 144 \
  --trials 5 --warmup 2 --repeats 10
```

For attribution, build the worker separately with `--features render-diagnostics`
and pass `profile_render: true` in its JSON input. Phases are returned as
`first_render_profile`. Rebuild without that feature before formal comparison.
The flags for image policy and preparation profiling retain their prior
semantics.

Local evidence is under ignored `output/text-raster-20261027/`: frozen native
workers, `before-full/`, `after-full/`, baseline and final diagnostic profiles,
`paper-sample.txt`, native comparisons, frozen browser hashes, build/test logs
and alternating native comparisons. A preliminary attribution run overlapped
browser QA and is retained as `profile-after.json`; it is not used for speed
claims. `profile-before.json` supplied the baseline table, while final profile
counts verify fast-path coverage.

## Full comparison and alternating attribution

The full synthetic corpus contains 14 files at 72/144 DPI. Both versions
completed all **420 timing jobs** successfully (five trials per engine/case/DPI),
plus separate image validation. Both retain **22/28** quality gates and the
same **20 eligible nonblank combinations** for ranking. Failures remain dense
Latin 72, rectangles 72, large JPEG 72/144 and dense CJK 72/144. No threshold
was relaxed, and failed/blank cases remain excluded from speed ratios.

| Metric | Previous rzpdf / PDFium | Current rzpdf / PDFium | Eligible combinations |
|---|---:|---:|---:|
| First output | 2.161 | 2.016 | 20 |
| Warm replay | 0.693 | 0.597 | 20 |
| Next page | 1.400 | 1.045 | 4 |

These are geometric mean relative engine timings from two serial runs at
different times. Some absolute case medians drifted upward between the runs,
so the entire ratio change must not be attributed to the optimization. First
output remains approximately twice PDFium in the broader matrix. Next-page
evidence is limited to four combinations.

To isolate the code change, a separate native-only comparison alternated
frozen old/current default workers, five fresh processes per case/DPI, two
warmups and ten measured replays. It covers seven synthetic text cases plus
the real paper at two DPIs: **160 successful processes**, 16 combinations and
40 byte-identical page outputs. Builds/browser checks did not run during
formal full or alternating timing. All image policies were default/budgeted.
Dense CJK can be measured old-versus-new here while remaining excluded from
cross-engine ranking because its existing cross-engine quality gate fails.

Geometric mean current/previous ratios for these 16 text combinations:

- First output: **0.961**, about **3.9% less time**.
- First replay alone: **0.954**, about **4.6% less time**.
- Warm output: **0.774**, about **22.6% less time**.
- Next-page output: **0.867**, about **13.3% less time**, six combinations
  including the real multi-page paper.

Selected alternating native medians at 144 DPI, milliseconds:

| Document | First old → current | Warm old → current | Replay-phase peak RSS old → current (MiB) |
|---|---:|---:|---:|
| text-dense | 35.58 → 33.00 | 7.43 → 6.29 | 23.09 → 23.12 |
| text-multipage | 32.98 → 28.01 | 4.97 → 3.97 | 22.61 → 22.54 |
| ttf-text | 27.15 → 27.70 | 5.49 → 4.71 | 21.47 → 21.54 |
| cjk-dense | 33.08 → 30.82 | 8.86 → 7.09 | 21.76 → 21.84 |
| cjk-multipage | 32.23 → 28.28 | 5.79 → 6.86 | 21.38 → 21.49 |
| paper | 64.18 → 63.12 | 11.99 → 10.58 | 25.78 → 25.73 |

The change is primarily a replay optimization, not a substantial memory
reduction. First-run glyph-mask creation and CoreText initialization remain;
no universal first-output speedup is claimed.

### Resolving short-loop variation

The short paired CJK multi-page 144 DPI warm median rose from 5.79 to 6.86 ms,
while the full-run median moved in the opposite direction. A follow-up
alternated three processes per version with ten warmups and 100 measured
replays at the affected cases. Whole-worker replay CPU was also measured.

| Case / DPI | Warm old → current (ms) | CPU/iteration old → current (ms) |
|---|---:|---:|
| CJK multi-page / 72 | 1.75 → 1.09 | 1.82 → 1.19 |
| CJK multi-page / 144 | 11.32 → 10.22 | 11.38 → 10.05 |
| TrueType / 144 | 9.55 → 8.05 | 9.37 → 7.74 |

The short-loop increase was not sustained in this recheck. Absolute times
vary substantially on this shared machine; raw results are preserved in
`warm-recheck.json`. These limited samples are not confidence intervals.

The real paper's pixels remain unchanged, so its previously recorded 72 DPI
page-4 MAE failure against PDFium also remains. Scan target-resolution policy
results from the preceding round are separate; this round does not relabel
those image-quality tradeoffs as default-mode gains.

Frozen default-build worker SHA-256:

```text
before 4017581aad02073f503be1b13a9214698653001e9e501840b082408ef039cc7d
after 64d168fdb46ca7801b790ab6c6b160859b60d922b7f5cae22f96e0f3305bb36d
```

## Limits and rollback

First-output performance is still approximately 2× PDFium on the broader
eligible corpus. The remaining CoreText initialization and first glyph-mask
work, plus canonical curve coverage generation, are substantial targets for
future attribution. Complex transforms, shaders, non-opaque fills and large
vector glyphs continue to use the original implementation. Resource accounting
and pixel equivalence take priority over broader batching eligibility.

The run dispatch is isolated in `glyph_run.rs` and its one caller: reverting
this round restores the independent-glyph path without a PDF data migration
or API change. The optional replay diagnostic feature and benchmark report
fields do not affect saved files, the WASM ABI or SDK options.
