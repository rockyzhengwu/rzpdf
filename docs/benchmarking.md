# Native engine performance comparison

The benchmark compares rzpdf's Skia backend with MuPDF and PDFium through their
native Python bindings. It measures rendering to an **owned RGBA8 buffer on
white**, without PNG encoding or file writes inside the timers. It does not
measure WebAssembly, text extraction, editing, saving, or GPU rendering.

See the [initial measured baseline and findings](benchmark-baseline-2026-10-26.md).
The [optimization results](render-optimization-results.md) contain the corrected
MuPDF comparison, fixes, before/after timings and browser regression evidence.
The [second optimization round](render-performance-round2.md) adds further
same-output improvements and separate phase CPU/RSS diagnostics.
The [render-session architecture and results](render-session-optimization.md)
document WASM resource reuse and memory boundaries.
The [comparison of committed code with PDFium](pdfium-comparison-2026-10-26.md)
records the committed baseline after the preparation changes.
The [font preparation and backend follow-up](font-preparation-performance.md)
records compact Identity CMaps, lazy shared
font managers and the remaining macOS first-initialization cost.
The [canonical coverage blending follow-up](coverage-blending-performance.md)
records the exact-output shortcut for solid coverage
on native raster targets, with browser verification and fallback boundaries.
The [first-render and real-document follow-up](first-render-performance.md)
contains the latest ranking, bounded CoreText initialization, mask-generation
measurements and the separately licensed real-file corpus and its limits.
The [page preparation follow-up](page-preparation-performance.md) adds exclusive
phase diagnostics and interleaved native before/after results; it does not rerank
the other engines.
The [failure and optimization backlog](render-optimization-plan.md) tracks
reproducers, priorities, investigation directions and acceptance criteria.

## Run

Use a quiet machine on a fixed power profile. Run engines serially; do not build
or run another benchmark at the same time. The first renderer build can take
several minutes. Subsequent benchmark progress prints once per case/DPI pair;
each worker has a 60-second timeout.

```sh
# A dedicated Python environment is recommended. These are the versions used
# for the initial macOS run; the worker records actual engine versions.
python3 -m venv .venv-benchmark
.venv-benchmark/bin/pip install PyMuPDF==1.26.7 pypdfium2==5.3.0 Pillow==12.2.0
cargo build --release --locked -p render --example benchmark_render
.venv-benchmark/bin/python scripts/prepare_benchmark_corpus.py output/benchmark-corpus
.venv-benchmark/bin/python scripts/benchmark_pdf.py \
  --manifest output/benchmark-corpus/manifest.json \
  --output output/benchmark-run --dpi 72 144 \
  --trials 3 --warmup 2 --repeats 10
```

Add `--diagnostics` to record process CPU and RSS at phase boundaries. The
operation timers retain the same boundaries; OS RSS queries run outside them.
The optional fields in `raw.jsonl`/`summary.json` include initialized RSS,
after-first and after-replay RSS, and CPU/wall ratios for first page and the
whole replay loop (including warmups and buffer destruction). CPU time sums
user/system time across process threads. A sustained ratio substantially above
one suggests multicore execution; a ratio near one is not proof that no other
thread ever ran. No CPU affinity or internal thread limit is forced.

Current RSS comes from `ps`, with missing values represented as null. Peak RSS
still comes from the OS high-water counter. The initialized baseline includes
each adapter/runtime. Differences from that baseline include lazy code pages
and allocator behavior as well as rendering resources; they are not pure
engine allocation counts. Diagnostic queries also add process overhead outside
the timed operations, so compare runs with the same diagnostics setting.
For before/after rankings against an older run without diagnostics, keep the
main timing run without `--diagnostics` and run diagnostics separately. Even
queries outside operation timers can perturb caches, scheduling and CPU clocks.

The output directory must be new. Python and native dependencies retain their
own licenses. MuPDF's licensing applies to its engine and binding. No Poppler
or network service is used.

The generator creates 14 deterministic synthetic cases: an explicitly blank
baseline; embedded Type1 sparse/dense/multipage Latin text; embedded TrueType
Latin text; sparse/dense/multipage Chinese text; dense rectangles; cubic curves;
overlapping transparency; small and 8.75-megapixel JPEG images; and a lossless
RGB image. Pages are 600 × 800 points. At 72/144 DPI, exact integer dimensions
avoid ambiguity about rounding. The fonts already exist in the repository;
see their fixture licenses and `font-data/NOTICE.md`. Generated files remain
under the ignored `output/` directory.

For your own corpus, supply a manifest:

```json
{
  "schema": 1,
  "cases": [
    {"name": "report", "path": "/absolute/path/report.pdf", "nonblank": true}
  ]
}
```

Paths can also be relative to the manifest. An optional `sha256` is verified
before execution; every actual input hash is recorded. Case names must be unique.
The first five pages are exercised. `nonblank` applies to all of those pages;
split mixed blank/nonblank documents into separate inputs when necessary.
Private inputs are never uploaded, but their paths and rendered images appear
in local output: review before sharing a report.

Use `--case ttf-text cjk-dense` to run a smaller named subset without changing
the manifest.

## Measurement boundaries

Each trial uses a fresh worker process. Interpreter imports and engine/renderer
initialization precede the measured operations. File-system caches are not
flushed, so **first page means a new document in an initialized engine**, rather
than cold disk access or end-to-end process startup.

| Metric | Included work |
|---|---|
| First page | Open document, obtain page count, prepare page 0, rasterize, copy owned RGBA |
| Warm replay | Replay explicitly retained page representation, rasterize, copy owned RGBA |
| Next page | First preparation and rendering of each remaining page, with document retained |
| Worker peak RSS | OS process peak, including language runtime, adapter and native engine |
| Process wall | Entire subprocess, including startup/imports/JSON; diagnostic only |

rzpdf retains a `PreparedPage` and renderer, MuPDF retains a `DisplayList`, and
PDFium retains a page handle. Each engine keeps its normal internal caches.
The MuPDF adapter creates its RGBA pixmap with a **null separation set**, matching
the standard display-list rendering helper; a solid DeviceRGB test checks this
against the normal PyMuPDF API. A non-null empty set in the installed binding
selects a different conversion path and must not be used as a default baseline.
These representations differ: **warm replay is not a claim that each engine
has an equivalent cache, or that the browser SDK would hit a cache**. In
particular, an explicit retained Type3 snapshot differs from document cache
eligibility. Annotations, widgets and JavaScript are excluded. Default optional
content visibility, crop, rotation and native resource limits apply.

Preparation and raster stage times are recorded to aid diagnosis. Lazy work
falls into different stages across engines; compare totals rather than ranking
the preparation stage alone. Python binding calls and owned-buffer copies are
part of the measured API cost. Results do not isolate a pure C/Rust raster loop.

The OS peak is collected with `/usr/bin/time` on macOS/Linux. If unavailable,
RSS is null. Python runtime/import overhead affects its baseline; this metric
is **not engine-only memory allocation**. Logical resource budgets are not RSS.
No memory limits are raised for benchmarks; resource errors remain failures.

## Quality gate and reporting

Before timing trials, each engine renders all selected pages to raw RGBA and
PNG for inspection. All three pairwise comparisons must agree on dimensions,
normalized mean RGB error ≤ 0.03, ink-mask F1 ≥ 0.97 with one-pixel tolerance,
and ink-area ratio between 0.75 and 1.3334. Expected nonblank pages cannot pass
as two blank outputs. The blank baseline has an explicit exemption.

These thresholds tolerate common antialiasing differences. They are diagnostic
checks, not a conformance test or proof that small colored details are correct.
Inspect PNGs, especially failures, before relying on the results. Differing
rounding/cropping of external documents can fail the dimension check; images
are never resized to make them pass.

Failures, traps and timeouts stay in `raw.jsonl`/`quality.json`. Successful
timings from a quality-failing case remain visible but are excluded from speed
ratios. A single failed timing trial excludes that engine/case. Ratios use only
the common subset with all engines passing every trial and the quality gate.
Numbers greater than one mean rzpdf is slower than the reference engine.

`report.md` contains per-case medians and paired geometric-mean ratios;
`summary.json` also includes warm P95 when there are at least 20 observations.
Warm observations within a process are correlated, so P95 is descriptive, not
a confidence interval. First-page medians use the trial count (three by
default). Next-page medians pool pages/trials and are null for single-page
inputs. Blank pages appear as their own baseline; use per-case figures when
choosing optimizations rather than treating the aggregate as a universal score.
Explicitly blank baselines are excluded from aggregate speed ratios.

`metadata.json` records platform, Python, revision, dirty status, renderer binary
hash and corpus hashes. `raw.jsonl` records actual MuPDF/PDFium versions, trial
order and individual samples. Stderr, including OS memory accounting and native
diagnostics, is retained separately. The fixed randomized engine order reduces
consistent order bias; it cannot remove thermal drift or OS scheduling noise.

## Verification and next scope

```sh
python3 -m unittest discover -s scripts -p test_benchmark_pdf.py -v
cargo fmt --all -- --check
cargo clippy --locked -p render --example benchmark_render -- -D warnings
```

Start with this reproducible baseline. A broader production corpus should add
Type3/CFF, ICC groups, soft masks, JPX/JBIG2, rotated/cropped pages, large page
counts and high-DPI regions, with visual review and permitted redistribution.
A separate browser benchmark is needed for WASM initialization, tiles, worker
messages and memory pressure. Neither those workflows nor broad PDF support
can be inferred from these synthetic native rendering scores.

API references: [MuPDF display lists](https://pymupdf.readthedocs.io/en/latest/displaylist.html),
[PDFium public rendering API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdfview.h),
and [pypdfium2 bitmap ownership](https://pypdfium2.readthedocs.io/en/stable/python_api.html).

The [image ownership and raster extent round](image-memory-performance.md)
removes a full RGBA copy, reduces the real scan's paired native peak RSS by
about 32 MiB, and unifies fractional-page dimensions across native/WASM/SDK.
It records the remaining PLOS 72 DPI image-quality failure explicitly.
