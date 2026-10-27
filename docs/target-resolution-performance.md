# Optional target-resolution JPEG decoding

The subsequent [text-replay round](text-replay-performance.md) refreshes the
complete synthetic ranking and optimizes exact-output glyph submission. This
report retains the preceding image-policy measurements and their limitations.

Rendering now has an explicit JPEG quality policy in native preparation,
WASM, the SDK and Worker. Existing calls keep their default behavior.
This round follows the pixel-preserving
[RGBA ownership optimization](image-memory-performance.md).

## Contract

- `budgeted` / Rust `ImageDecodePolicy::Budgeted` preserves full image pixels
  when they fit, retaining the previous reduced-decode fallback when necessary.
- `target` / Rust `ImageDecodePolicy::TargetResolution` proactively chooses
  reduced JPEG IDCT covering both transformed image axes at the requested DPI.
  This avoids generating the full-resolution samples and RGBA first.
- Sequential external JPEGs can use 1/8, 1/4 or 1/2 decoding. Masks, SMask/Matte,
  progressive JPEG, unsupported filters/parameters, inline images and JPX
  retain the original bounded full-resolution paths. Patterns and Type3
  image programs retain their original precision.
- If full resolution is needed, the existing full path is used. Source, pixel,
  decoded-byte and render limits are unchanged. Unsupported reduction cannot
  turn an over-budget full decode into an unbounded allocation.
- Saving and ordinary extraction keep original source precision. Target mode
  can change detail pixels, so it is an application-selected display/preview
  quality policy rather than a pixel-equivalent speedup.

```ts
const image = await doc.render(0, 144, { imageDecode: 'target' });
const tile = await doc.renderRegion(0,
  { x: 0, y: 0, width: 512, height: 512 }, 576,
  { imageDecode: 'target' });
```

```rust
use pdf::document::prepared_page::ImageDecodePolicy;
let page = doc.prepare_page_for_render_with_policy(
    0, None, 144., ImageDecodePolicy::TargetResolution)?;
page.replay(&doc, &mut renderer)?;
```

Prepare again at the new DPI when zoom changes. Replay of an already reduced
snapshot cannot restore discarded detail. A tile uses the whole image's draw
scale, not just its visible rectangle, so neighboring tiles share samples.
The SDK and Worker validate and snapshot the option; native WASM flags bit 4
selects target mode, leaving annotation/rotation bits unchanged. Font preflight
accepts and reports the same policy. Capabilities expose `supportedPolicies`
and `defaultPolicy` under `imageDecode`.

## Cache architecture

Prepared-page identity includes document/revision, DPI, image policy and
annotation options. Switching quality never reuses a snapshot decoded for the
other policy. Per-interpreter target-image entries also check the linear draw
transform: a full-size draw must not force a later small draw to reuse its
full samples. Translation alone can reuse decoded pixels. This cache retains
one variant per image name and remains bounded by the existing entry and
sample-byte limits; it does not accumulate every zoom variant.

The default policy retains its prior full-image sharing. Backend image caches
continue to charge retained raster bytes. Clearing caches releases engine
references; caller-owned buffers and WASM linear-memory capacity have separate
lifetimes. A target request following an earlier full decode is not guaranteed
to undo the earlier process peak or immediately release all prior buffers.

## Verification

- Six image-resolution tests passed: opt-in policy, DPI/policy cache identity,
  source-preserving save/extraction, large/small/rotated repeated draws,
  translation reuse, native-resolution fallback, resource guards, progressive
  fallback and masked/Matte full-resolution retention.
- Seven prepared-page tests, 113 render tests and 24 WASM interface tests passed.
  The WASM tests include font preflight acceptance/rejection of the new option.
- SDK/protocol and Worker tests passed, including stable flag encoding,
  invalid policy rejection and immutable queued policy options.
- All 28 synthetic native document/DPI combinations retained their 44 previous
  byte-exact page outputs in default mode.
- Pinned release WASM and TypeScript builds passed. Browser default regression:
  56 synthetic and eight real combinations passed; ICC/Type3 and resource
  suites passed in both direct and Worker transports.
- New actual-WASM policy fixture passed in both transports: full/target cache
  identity, stable replay and clear, full-versus-stitched tiles, high-DPI
  full-resolution fallback, preflight and handle release. Its retained page
  estimate falls from 11,522,096 to 182,096 bytes. These are estimates, not RSS.
- Clippy passed for render/web-wasm all targets with Rust warnings denied;
  six Python benchmark harness tests passed. Existing macOS mozjpeg assembly
  linker messages are unchanged.

The project-authored fixture is reproducible without downloads:

```sh
python3.11 scripts/prepare_target_decode_fixtures.py
npm --prefix web run build
RZPDF_CHROMIUM='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  node web/tests/image-decode.mjs
```

## Measurement method

The fixed-hash PLOS paper and NARA scan from
[the licensed real corpus](benchmark-real-corpus.json) were tested at 72 and
144 DPI. Each combination used five fresh processes per variant: frozen prior
worker, current budgeted, current target and PDFium. Variant order rotated by
trial. Each process ran two warmups and ten measured replays. Builds and
browser tests were completed before this serial run. Timers exclude output
serialization; diagnostic RSS sampling occurs outside operation timers.
Warm figures are medians of each process's median. Peak RSS is a whole-worker
observation, not an engine-only allocation count; PDFium includes Python adapter
overhead. The old/current default comparison establishes unchanged behavior;
the target comparison intentionally changes decoding quality.

Local artifacts: `output/target-decode-20261027/`, including frozen worker,
`paired.py`, `paired.json`, native output verification, quality precheck,
reviewed scan detail crop, browser reports, build reports and test logs.

## Results

All 80 fresh-process runs succeeded. Median results follow; first and warm are
milliseconds, peak RSS is MiB sampled after the replay phase.

| Document / DPI | Variant | First output | Warm replay | Peak RSS |
|---|---|---:|---:|---:|
| constitution-scan / 72 | before | 214.71 | 39.25 | 73.52 |
| constitution-scan / 72 | budgeted | 219.73 | 44.34 | 73.52 |
| constitution-scan / 72 | target | 113.73 | 3.71 | 21.19 |
| constitution-scan / 72 | pdfium | 183.91 | 35.16 | 55.98 |
| constitution-scan / 144 | before | 228.95 | 44.67 | 88.38 |
| constitution-scan / 144 | budgeted | 244.46 | 49.79 | 88.36 |
| constitution-scan / 144 | target | 156.98 | 14.90 | 55.59 |
| constitution-scan / 144 | pdfium | 209.89 | 59.66 | 67.19 |
| plos-data-sharing / 72 | before | 61.57 | 7.62 | 14.82 |
| plos-data-sharing / 72 | budgeted | 58.37 | 6.92 | 14.72 |
| plos-data-sharing / 72 | target | 55.89 | 7.27 | 14.80 |
| plos-data-sharing / 72 | pdfium | 32.89 | 6.08 | 21.73 |
| plos-data-sharing / 144 | before | 88.09 | 15.91 | 26.11 |
| plos-data-sharing / 144 | budgeted | 79.92 | 17.16 | 26.07 |
| plos-data-sharing / 144 | target | 87.82 | 14.88 | 26.06 |
| plos-data-sharing / 144 | pdfium | 41.58 | 15.62 | 33.27 |

For the 144 DPI scan, current target versus current budgeted reduces median
first output by 35.8%, warm replay by 70.1% and peak RSS by 37.1%. At 72 DPI,
first output falls by 48.2% and peak RSS by 71.2%. Decode/conversion workloads
are smaller, and the reduced raster now fits backend image caching. These
results include the cache benefit; they do not attribute all savings to IDCT.
The mostly text-based paper has no reduced eligible images and no expected
policy-related improvement; its target/default output is byte-identical.

PDFium used its existing adapter settings. The target-mode scan timings are
competitive in this run, but changing image decode policy changes the work
and output, so this is not evidence that default rzpdf universally matches or
beats PDFium at identical quality. Browser tests verify correctness and
retention estimates; this report does not claim WASM timing improvements.

### Quality gates

All old/current default real-document outputs are byte-identical, as are
all target/current default PLOS outputs. The NARA scan changes pixels:

| DPI | Target vs budgeted normalized RGB MAE | Target vs PDFium MAE | Target vs PDFium ink F1 |
|---|---:|---:|---:|
| 72 | 0.028734 | 0.014547 | 1.000000 |
| 144 | 0.017678 | 0.013867 | 1.000000 |

Both scan cases pass the existing MAE ≤ 0.03, ink F1 ≥ 0.97 and ink-ratio
checks against both references. A handwritten-detail crop was also reviewed:
the reduced preview smooths fine strokes relative to full nearest sampling.
The thresholds measure agreement, not preservation of every fine detail or
PDF conformance. For precision work keep the default policy or render at a
higher DPI.

The previously documented PLOS 72 DPI page-4 difference remains: target and
default are identical, but normalized RGB MAE versus PDFium is 0.030387,
above the unchanged 0.03 ceiling. It remains excluded from gated comparisons.
No thresholds were relaxed.

Frozen native worker SHA-256:

```text
before 72afe8b05401fbf2f4472a492691b52faced145794dd737fe444e498f0ed47ba
after 4017581aad02073f503be1b13a9214698653001e9e501840b082408ef039cc7d
```

### Longer default-policy recheck

The short run showed variable default timings, including slower scan replay
medians for the new worker. To check for a sustained regression, a separate
serial run alternated old/current default workers at 144 DPI, with three fresh
processes each, ten warmups and 100 measured replays per process. No builds or
browser tests ran concurrently.

| Document | First old → current (ms) | Warm old → current (ms) | Replay CPU/iteration old → current (ms) |
|---|---:|---:|---:|
| Scan | 233.23 → 231.24 | 50.04 → 48.02 | 48.75 → 49.73 |
| Paper | 77.41 → 70.80 | 17.12 → 17.27 | 16.79 → 17.38 |

Default performance was broadly similar in this longer check; CPU values
still vary by a few percent. The short-run default slowdown was not a
consistent wall-time result, and no default-mode speed improvement is claimed.
Raw evidence is in `default-recheck.py` / `default-recheck.json`.

The native JSON benchmark worker accepts `"image_decode": "target"` (or
`"budgeted"`) in addition to its existing path, dpi, warmup and repeats fields.
It reports the selected policy in its result. The standard three-engine
benchmark script continues to use the default policy; target-policy timing
must be labeled separately and retain quality checks.
