# Image buffer ownership and raster extents

This round removes an image-sized copy between decoded PDF RGBA and Skia,
and makes fractional-page raster dimensions consistent across native, WASM
preflight and the JavaScript SDK. No resource limits were raised.

## Owned image pixels

Previously `do_image_ref` held the decoded RGBA `Vec` while
`Data::new_copy` allocated another complete RGBA buffer. The new
`owned_raster` module transfers the allocation into an immutable Skia pixel
reference. Its release callback returns ownership to Rust after the final
image reference is released, or if Skia rejects installation. Strides and
buffer lengths are checked before calling the generated native binding.
The safe Skia bitmap installer does not expose this release callback, so the
small unsafe boundary is isolated in that module.

Decoding, ICC/Matte/mask conversion, interpolation and image cache budgets
retain their existing behavior. Clones share the transferred base pixels;
Skia's mipmap construction makes its own base copy, so this change does not
remove all temporary image allocations. Tests check pointer identity,
release count, independent mipmap lifetime and rejected buffer layouts.

## Fractional extents

All three layers now allocate `ceil(f64(f32(size)) * f64(f32(dpi)) / 72)`
pixels per axis. This covers fractional page edges, including transformed
CropBox/UserUnit dimensions. Computing with the original DPI also avoids
an accidental extra pixel at common DPI: a 600 pt page at 96 DPI is 800
pixels, whereas first computing `f32(96 / 72)` and then applying ceil gives
801. Geometry transforms retain their previous precision. Full-page and
region limits still apply before allocation.

This is an intentional change from nearest-integer output dimensions for
fractional pages. Applications should use SDK `rasterSize()`. The PLOS
paper at 72 DPI changes from 612 × 791 to 613 × 791; the old 612-column
area is byte-identical on all five pages. Its 144 DPI outputs and both
scan outputs are byte-identical to the previous worker.

## Paired native measurements

macOS x86_64, release native worker, original fixed-hash public PDFs from
[the real corpus](benchmark-real-corpus.json). Each case/DPI used five
fresh processes per version in alternating order, with two warmups and
ten measured replays per process. Diagnostics sampled peak RSS outside
operation timers. Builds and browser checks did not run concurrently.
The table contains medians; warm values first take each process's median.
RSS includes the native worker and raster readback, not just decoder bytes.

| Document / DPI | First output before → after (ms) | Warm before → after (ms) | Peak RSS before → after (MiB) |
|---|---:|---:|---:|
| PLOS / 72 | 50.64 → 51.89 | 5.72 → 5.80 | 14.65 → 14.58 |
| PLOS / 144 | 65.30 → 64.43 | 12.49 → 12.34 | 25.93 → 25.83 |
| NARA scan / 72 | 253.38 → 212.22 | 49.51 → 35.20 | 105.61 → 73.66 |
| NARA scan / 144 | 246.04 → 216.20 | 56.70 → 41.77 | 120.32 → 88.20 |

The 2550 × 3300 scan eliminates about 32 MiB of peak RSS, matching one
full RGBA allocation. At 144 DPI that is a 26.7% peak reduction, 12.1%
first-output improvement and 26.3% warm-output improvement in this paired
run. The largely text-based paper shows no material speed improvement.
These are two documents, not a general performance claim.

Local reproducible artifacts live under ignored
`output/image-memory-20261027/`: frozen previous worker, alternating
`paired.py`/`paired.json`, build and test logs, current engine comparisons
and native/browser regressions. Earlier broader corpus results remain in
[first-render-performance.md](first-render-performance.md).

## Cross-engine results and validation

A fresh serial three-engine run completed all 60 timing jobs successfully
(five processes per engine for four document/DPI combinations). Image gates
remain **3/4**: the old PLOS 72 DPI dimension mismatch is fixed, but page 4
still differs from PDFium by normalized RGB MAE 0.030387 versus the unchanged
0.03 ceiling. Its ink F1 is 0.999718. This is recorded as a remaining text
rasterization discrepancy, not counted as an eligible timing win. All five
PLOS pages pass comparison against MuPDF at 72 DPI.

| Case | rzpdf first / warm (ms) | PDFium first / warm (ms) | rzpdf / PDFium peak RSS (MiB) |
|---|---:|---:|---:|
| Scan 72 DPI | 224.35 / 32.75 | 154.09 / 32.67 | 73.62 / 56.13 |
| Scan 144 DPI | 229.63 / 46.18 | 182.40 / 56.82 | 88.47 / 67.23 |
| Paper 144 DPI | 67.66 / 12.21 | 31.19 / 12.69 | 36.59 / 34.54 |

For these three eligible combinations only, geometric mean rzpdf/PDFium
first-output and warm ratios are 1.584 and 0.922. This does not replace the
larger synthetic-corpus aggregate. Cross-engine RSS includes each worker's
runtime and image adapter, so it is not an engine-only memory comparison.
Use the alternating same-worker measurements above to attribute the copy
removal. Remaining scan allocations include the full JPEG samples, converted
RGBA, surface and readback; target-resolution decoding remains a separate
opportunity where supported by the codec and masks.

Validation for this round:

- Native render suite: 112 passed; WASM interface suite: 24 passed.
  The subsequently added native-rejection release test and the two other
  ownership tests also passed together (3/3).
- All 28 synthetic document/DPI combinations produced 44 byte-identical
  native page outputs against the previous reference.
- Pinned release WASM build and TypeScript build passed. SDK resource and
  benchmark harness tests passed (six Python harness tests).
- Browser direct/Worker: 56 synthetic and 8 real combinations passed,
  including repeated render/cache clear and immutable outputs.
- Two newly generated fractional-page/CropBox/rotation/UserUnit fixtures
  passed 12 direct/Worker × 72/96/144 DPI full-versus-tile cases, with native
  comparison at 72 DPI. This is the targeted new region subset, not a claim
  that the entire historical region fixture matrix was rerun.
- ICC/Type3 and resource-profile browser suites passed in both transports.
- Clippy on render and web-wasm, all targets, with warnings denied passed.
  Existing macOS mozjpeg assembly linker messages remain outside this change.

Frozen release worker SHA-256:

```text
before 816accae95b447310d496cc918506713da30f2b397414e029f37e8b9f0f7b2e1
after  72afe8b05401fbf2f4472a492691b52faced145794dd737fe444e498f0ed47ba
```
