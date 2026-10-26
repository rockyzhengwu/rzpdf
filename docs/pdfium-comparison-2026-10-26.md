# PDFium comparison after page-preparation optimization

Measured code commit: `27717368bb8bf38edf4210e8119ef562c40ccc3a`.
The working tree was clean when the benchmark started. The native Release worker
matches the binary validated in the [page-preparation follow-up](page-preparation-performance.md).

## Results

| Operation | Previous rzpdf / PDFium | Current rzpdf / PDFium | Paired combinations |
|---|---:|---:|---:|
| First output after opening a document | 3.70x | **3.27x** | 20 |
| Warm replay of a retained page | 0.78x | **0.79x** | 20 |
| Subsequent pages of an open document | 1.74x | **1.33x** | 4 |

These are geometric means of paired case/DPI latency ratios; smaller is better.
A ratio of 0.79 means about 21% less elapsed time, not that every page is faster.
First output includes opening the PDF, preparing the first page, drawing and
returning owned RGBA pixels. Process startup and initial renderer creation are
excluded. It is not the cost of every newly visited page. Subsequent-page samples
come from two multipage files at two DPI settings, so that result has limited
coverage. Warm rendering explicitly retains the interpreted scene and session.

The preparation changes improve first output and page navigation. Cold rendering
still has a substantial gap, while warm performance remains in the previous
range. This run is independent of the previous run; the interleaved native A/B
experiment in the preparation report is the stronger evidence for attributing
changes to the implementation.

## Representative cases

Medians in milliseconds; 144 DPI. Each first-page median uses five independent
processes; each warm median pools the measured replays.

| Input | rzpdf first | PDFium first | rzpdf warm | PDFium warm |
|---|---:|---:|---:|---:|
| Dense Latin text | 67.38 | 21.05 | 9.89 | 9.52 |
| TrueType text | 50.32 | 12.52 | 5.88 | 4.78 |
| Curves | 143.33 | 25.24 | 47.21 | 20.42 |
| Rectangles | 63.88 | 23.20 | 19.88 | 17.36 |
| Small JPEG | 86.13 | 60.22 | 13.08 | 40.02 |

For the lossless image at 72 DPI, warm replay is 2.52 ms versus PDFium's 11.10 ms.
Image resource reuse is a strong case for rzpdf here. Curves remain substantially
slower even after caching, and first text rendering still includes expensive font
backend work. Do not interpret the overall warm ratio as equivalent performance
for those workloads or as representative of arbitrary real PDFs.

## Method and correctness

- Same 14 deterministic synthetic PDFs, with corpus SHA-256 verification.
- Full-page output at 72 and 144 DPI; five trials, two warmups and ten warm replays.
- Engines execute serially, with no builds, tests or browser checks running during
  timing. Resource diagnostics are disabled for this ranking.
- PDFium **145.0.7616.0**, pypdfium2 **5.3.0**. The native benchmark uses the installed
  binary and does not assume its internal renderer configuration.
- The runner also measures MuPDF **1.26.12**, through PyMuPDF **1.26.7**. Its current
  rzpdf/reference ratios are 2.35x first, 1.26x warm and 1.86x subsequent pages.
- All **420 timing trials succeeded**, without timeout or trap.
- The unchanged three-engine image gate passes **22/28 combinations**. Removing
  two blank baselines leaves the same **20 nonblank combinations** for rankings.
- All **44 selected rzpdf page outputs** have identical dimensions and RGBA bytes
  to the preceding render-session run. No quality gate was relaxed.

The six combinations still excluded from cross-engine speed ratios are dense
Latin text at 72 DPI, rectangles at 72 DPI, the large JPEG scan at both DPI values,
and dense Chinese text at both DPI values. These exclusions preserve the existing
image-difference findings; a gate is not a PDF conformance certificate.

Raw peak RSS includes each worker's runtime and adapter. PDFium and MuPDF run
through Python bindings, while rzpdf has a native executable. Their raw process
peaks are not a pure-engine allocation comparison. This timing run does not
repeat the earlier phase CPU/RSS diagnostics or establish browser performance.
The corresponding WASM build has separate correctness acceptance, described in
the preparation report.

## Evidence and reproduction

Artifacts: `output/benchmark-committed-2771736/`, including `metadata.json`,
`raw.jsonl`, `quality.json`, `summary.json` and the generated `report.md`.
The metadata identifies the measured commit, clean working tree, corpus hashes,
source hashes and worker hash. Worker SHA-256:
`b99d4ab22d59f62cd5ba5ba555e105dacb0d000a028d1ec6fe46cca324829187`.

```sh
cargo build --release --locked -p render --example benchmark_render
python3 scripts/benchmark_pdf.py \
  --manifest output/benchmark-corpus-v2/manifest.json \
  --output output/benchmark-new-run --dpi 72 144 \
  --trials 5 --warmup 2 --repeats 10
```

Use the benchmark Python environment documented in [benchmarking.md](benchmarking.md).
The output directory must be new. Build without `prepare-diagnostics` for formal
comparisons; diagnostic workers are for attribution, not ranking.
