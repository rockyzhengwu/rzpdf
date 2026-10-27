# Canonical coverage blending

Date: 2026-10-27. This follows the [font preparation round](font-preparation-performance.md).
The comparison binary is that round's final binary, including its uncommitted
Identity CMap and session font-manager changes. It is not `2771736` alone.

## Implementation and scope

A fresh thread sample of the dense-curve case placed 1,105 of 1,529 main-thread
samples under cached coverage rectangle submission, including 1,079 in
`SkCanvas::drawRect`. Retaining canonical coverage avoids generating it again,
but previously still submitted every small rectangle through the canvas.

`render/src/coverage_blend.rs` now applies solid SrcOver coverage directly to
the owned raster target. It uses the same two stages as Skia's integer solid
blitter: round the color's premultiplication by alpha, then add the destination
scaled by `(256 - alpha) / 256` with integer truncation. Coverage and paint
opacity are combined using the existing rounded division by 255.

Both freshly rasterized canonical cells and cached coverage use this path.
Curve subdivision, canonical cells, antialias samples, rectangle coalescing,
cache keys, work accounting and resource limits remain unchanged. There is no
additional persistent cache or whole-page intermediate image.

The fast path requires the backend's native N32 pixel order, premultiplied
alpha, an untagged target, a hard rectangular clip, the canonical integer
translation and plain solid SrcOver paint. Shaders, filters, dithering, path
effects, fractional antialiased clips, non-native pixel order and other
transforms retain canvas drawing. Huge coordinate ranges also retain the
canvas path, preserving its f32 placement semantics. Groups use separate root
surfaces; the helper is private and does not support canvas saveLayer targets.

The surface is notified before borrowing its pixels, preserving Skia snapshot
copy-on-write. Writes honor the device clip, tile origin and row stride.
The helper allocates no backing buffer. The canonical sample/work checks run
before either compositor, including on cache hits.

The native float-to-byte conversion is deliberate: `Paint::color()` in the
pinned Rust wrapper truncates components, whereas the native integer blitter
rounds them. The implementation uses `Color4f::to_bytes()` to preserve native
rounding. The non-native channel order also uses a different Skia compositor;
it is rejected by the shortcut rather than assumed equivalent.

## Verification

- 106 render library tests and 24 WASM interface library tests passed.
- New tests compare all 256 opacity values against all 256 coverage values,
  fractional source colors, varied rectangle widths, mixed destination alpha,
  snapshot retention, translated cells, hard clipping and fallback for
  fractional antialiased clips and non-native pixel order.
- All 44 selected native page outputs match the preceding round byte-for-byte.
- The pinned release WASM build passed. Direct/Worker browser regression
  passed all 56 combinations, checking cold output, cached replay and output
  after cache clearing. ICC/Type3 and resource regressions passed in both
  transports.
- Strict workspace/all-target Clippy passed with default features and with
  `pdf/prepare-diagnostics`; formatting, diff checks and six benchmark harness
  tests passed. This round did not rerun the unchanged PDF library's full suite.

## Paired performance

The original 14-case corpus at 72/144 DPI was run with five interleaved trials
per binary, two warmups and ten warm replays: 280 successful trials and 44
byte-identical selected page outputs. The two binaries were selected in a
deterministic shuffled order within each pair. Builds, tests and browser
checks finished before timing started. Other user applications were not
stopped; no CPU affinity or fixed power profile is claimed.

Geometric mean after/before across the original 26 nonblank combinations:

| Phase | Ratio | Change |
|---|---:|---:|
| Preparation | 0.998 | Essentially unchanged |
| First raster phase | 0.965 | 3.5% less time |
| First output | 0.970 | 3.0% less time |
| Warm replay | 0.862 | 13.8% less time |

On the same 20 nonblank combinations eligible for cross-engine ranking,
first output took 2.9% less time and warm replay 13.2% less time.

Selected medians in milliseconds:

| Case | DPI | First before | First after | Warm before | Warm after |
|---|---:|---:|---:|---:|---:|
| Curves | 72 | 72.10 | 56.23 | 20.78 | 5.59 |
| Curves | 144 | 132.17 | 97.97 | 44.19 | 11.89 |
| Dense rectangles | 72 | 34.72 | 27.26 | 11.01 | 3.70 |
| Dense rectangles | 144 | 54.29 | 44.52 | 16.10 | 8.97 |

The 144 DPI curve case reduced first-output time by 25.9% and warm replay by
73.1%. That workload-specific result is substantially larger than the overall
aggregate. Text and image cases mostly bypass this particular bottleneck.

The five-trial paired run showed a 15.4% first-output slowdown for transparency
at 144 DPI. A separate 20-trial interleaved recheck (40 runs) did not reproduce
it: first-output after/before was 0.982 and warm replay 0.985. This specimen
uses the existing direct rectangle path rather than canonical coverage. The
initial outlier is retained in the raw evidence.

## Fresh engine comparison

Five trials, two warmups and ten warm replays at both DPIs produced 360
successful three-engine timing trials. This run omitted `jpeg-scan` and
`cjk-dense`, whose four combinations had already failed the previous image
agreement gate and contributed no eligible timing pairs. Both remain in the
full native before/after validation above. The generator's original 14 cases
and quality thresholds have not changed.

The selected 12-case comparison passed 22/24 image gates; the remaining failures
are still excluded. After removing two blank combinations, the ranking uses
exactly the same 20 eligible nonblank combinations as the preceding round.
All 40 selected native page outputs in this run match the preceding round.

Geometric mean of rzpdf time / reference time, with lower values better:

| Phase | PDFium | MuPDF |
|---|---:|---:|
| First output | 2.88× | 2.15× |
| Warm replay | 0.64× | 1.02× |
| Next page | 1.35× | 1.82× |

The preceding round's corresponding PDFium ratios were 2.99×, 0.80× and
1.30×. Next-page evidence is only four paired combinations and does not show
an improvement in this separate comparison. The interleaved native experiment
is the stronger evidence for the isolated code change.

At 144 DPI, the curve case's fresh rzpdf/PDFium medians were 98.15/22.44 ms for
first output and 11.40/17.14 ms for warm replay. Warm coverage reuse is faster
on this specimen, while first coverage generation remains substantially slower.
This is not a claim of general PDFium parity.

Versions remain PDFium 145.0.7616.0 / pypdfium2 5.3.0 and MuPDF 1.26.12 /
PyMuPDF 1.26.7. These are native owned-RGBA measurements, not WASM timings or
a claim about arbitrary real documents. See [methodology](benchmarking.md).

Reproduce the selected comparison after building the release worker:

```sh
/usr/local/bin/python3.11 scripts/benchmark_pdf.py \
  --manifest output/benchmark-corpus-v2/manifest.json \
  --output output/coverage-comparison \
  --case blank text-sparse text-dense text-multipage rectangles curves \
    transparency jpeg-small flate-image ttf-text cjk-sparse cjk-multipage \
  --dpi 72 144 --trials 5 --warmup 2 --repeats 10
```

## Separate resource diagnostics

After timing, three diagnostic trials per binary and case produced another
24 successful native runs at 144 DPI. Peak RSS below is the maximum of those
three whole-worker peaks, in MiB; it is not a precise allocation counter.

| Case | Before | After |
|---|---:|---:|
| Curves | 28.56 | 28.46 |
| Dense rectangles | 22.41 | 22.32 |
| Embedded TrueType | 21.37 | 21.41 |
| Small JPEG | 34.76 | 34.70 |

These checks show no material peak-RSS increase on the selected cases.
Warm-loop process CPU/wall ratios were approximately 0.999 for both binaries;
no sustained multicore advantage was observed. Neither observation guarantees
all documents behave the same way. Diagnostic times were excluded from speed
rankings. The three-engine report's PDFium/MuPDF RSS includes their Python
adapter runtimes and cannot be interpreted as pure engine memory.

## Remaining first-font cost

The macOS first system-font enumeration remains inside first-render timing.
The public Skia native factory accepts a specified font collection, but the
pinned Rust font-manager binding does not expose that argument. See the
[native factory](https://skia.googlesource.com/skia/%2B/b988ee43e367/include/ports/SkFontMgr_mac_ct.h)
and the preceding round's local source/profiling evidence. A binding extension
remains a separate task; this round preserves CoreText and the existing font
selection behavior.

## Local evidence

Artifacts are under ignored `output/coverage-batch-20261027/`: frozen before
and after binaries, `curves-before.sample.txt`, native validation outputs,
paired-run driver/raw records, browser reports and build/test logs.

```text
before 72c7c5c2cf0a8249eb98cb42762c30a0688df70c53a1a6624d10fd3817e27733
after  84df63d121e77a5a8f5ab413f75bf2ef70d69dc7650996a691962a0c4761c2c2
```
