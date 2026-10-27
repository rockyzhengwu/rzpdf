# First-render optimization and real-document baseline

Subsequent image-buffer ownership and fractional-page extent results are in
[image-memory-performance.md](image-memory-performance.md). That round reduces
scan peak RSS without changing decoded pixels; the measurements below remain
historical evidence for the earlier optimization round.

Date: 2026-10-27, macOS x86_64. Baseline implementation: `fd6ded0`.
Changes are split into `eae7712` (bounded CoreText initialization), `53be974`
(coverage masks), and `4eaa38d` (the named Zapf encoding alias).

## Results

The same 20 eligible nonblank synthetic combinations now take **2.14× PDFium
time for first output**, compared with 2.88× in the preceding round. Warm
replay is 0.67× versus the preceding 0.64×; next page is 1.24× versus 1.35×,
with only four paired next-page combinations. First output still has a
substantial gap; these numbers do not establish parity on arbitrary PDFs.

| Fresh native comparison | rzpdf / PDFium | rzpdf / MuPDF |
|---|---:|---:|
| First output | 2.140 | 1.559 |
| Warm replay | 0.667 | 1.081 |
| Next page | 1.242 | 1.765 |

Five trials, two warmups and ten warm replays at 72/144 DPI produced 360
successful three-engine timing trials. The selected 12-case comparison passed
22/24 image gates. As in the preceding round, `jpeg-scan` and `cjk-dense`
were omitted from this ranking run because their four combinations had already
failed agreement and contributed no eligible timing pairs. Both remain in
the full native validation. Thresholds and the generator's 14 cases are unchanged.

First output includes opening, preparation, first replay and owned RGBA
readback. Backend font initialization remains inside replay. Renderer/process
initialization, PNG encoding and file writes are excluded consistently with
[the benchmark methodology](benchmarking.md). Versions remain PDFium
145.0.7616.0 / pypdfium2 5.3.0 and MuPDF 1.26.12 / PyMuPDF 1.26.7.

## Isolating the changes

The [CoreText comparison](coretext-font-performance.md) contains 140 interleaved
font-specific runs with 30 byte-identical page outputs. First output took
38.2% less time across those 14 text combinations. The improvement removes
Skia's explicit system-family enumeration, while retaining an initial UI-font
query and the same byte loader. Other platforms do not inherit this measured
macOS font-initialization gain.

Solid canonical masks now use native N32 instead of Alpha8, so Skia can use its
integer solid blitter. Only alpha is read. The fixed 68×68 canonical viewport,
curve normalization, work accounting, cache keys and saved rectangles are
unchanged. Shader and non-SrcOver image-mask semantics retain Alpha8. Unused
mask snapshots are avoided for solid spans.

This increases one transient mask's actual backing by about 14 KiB; the
existing scratch preflight already reserves two N32 cells. No resource cap
or persistent cache was enlarged. Alpha equivalence is checked against A8
on overlapping contours and holes with both winding rules; padded N32 rows
also reconstruct exactly the same coverage rectangles as A8.

A separate 40-run interleaved curve experiment used ten trials per binary
and DPI, each with ten replays after clearing `RenderSession`. It retained
the same prepared display list and excluded cache clearing itself from timers.
This diagnostic is distinct from warm replay and from fresh-document timing.
Paired cold-replay ratios were **0.964 at 72 DPI and 0.978 at 144 DPI**,
approximately 2–4% less time. The remaining analytic coverage cost is large.
Unhelpful curve-math and allocation experiments were not retained.

## Full native before/after check

Five interleaved trials per binary across all 14 original cases and both DPIs
produced 280 successful runs and 44 byte-identical selected page outputs.
With one warmup and three warm replays, the 26 nonblank combinations showed
first-output ratio **0.758**, or 24.2% less time. Preparation was essentially
unchanged (0.994); first raster phase was 0.729.

That short warm loop showed multiple >10% timing outliers and a 3.8% aggregate
warm slowdown. It was followed by a 240-run interleaved recheck of all
nonblank cases except the large, previously ineligible JPEG scan, using the
regular two warmups and ten warm replays. All 40 selected page outputs were
identical; none of the first/warm case medians regressed more than 10%.
On the same 20 eligible combinations, first-output ratio was 0.737 and warm
ratio 1.014. Warm performance is broadly unchanged; it is not claimed to have
improved this round. All initial outliers remain in the raw records.

Builds, unit tests and browser checks finished before timing runs; native,
cross-engine, recheck and diagnostic batches ran sequentially. Other user
applications remained active. No CPU affinity or fixed power profile was
enforced, and separate runs have different absolute times.

## Real documents and the compatibility finding

[The pinned real corpus](benchmark-real-corpus.json) contains two original
files, kept separately from the synthetic ranking:

- Piwowar, Day and Fridsma's 2007 PLOS ONE paper, five pages and nine font
  objects, with text, tables and figures. The
  [publisher states its Creative Commons Attribution permission](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0000308).
- The National Archives' original Constitution classroom scan PDF, including
  its captions. The [source workshop states that NARA educational materials
  are public domain](https://www.archives.gov/education/lessons/constitution-workshop).

Only source/provenance/hash metadata and the downloader are committed; the
original PDFs and renders stay in ignored output storage. No input was
rewritten to remove an unsupported feature or improve an agreement score.

The scan initially failed in both baseline and optimized rzpdf with
`Font encoding ZapfDingbatsEncoding`. The engine already had the complete
Zapf mapping. `4eaa38d` adds this exact producer alias to that mapping, with
all 256 glyph names/Unicode entries checked and unknown names still rejected.
The original failing evidence is retained. This is a compatibility fix, not
a measured speedup over the failing baseline.

After the fix, all 60 real-corpus timing trials succeeded. Three of four
case/DPI combinations passed three-engine image gates. The paper at 72 DPI
is excluded: rzpdf rounds its fractional page width to 612 pixels while
PDFium/MuPDF produce 613. The 144 DPI dimensions agree. No threshold or
dimension-normalization workaround was applied.

Selected eligible 144 DPI medians, milliseconds:

| Original document | First rzpdf | First PDFium | Warm rzpdf | Warm PDFium |
|---|---:|---:|---:|---:|
| PLOS paper | 63.75 | 30.34 | 11.63 | 11.20 |
| NARA scan | 241.67 | 181.59 | 55.37 | 56.10 |

A further 20-run interleaved paper-only comparison measured first output at
66.18→50.33 ms (72 DPI) and 80.42→64.27 ms (144 DPI), about 20–24% less time.
All ten selected paper page outputs match the baseline byte-for-byte.
The real sample is two documents, not evidence for a general real-world ranking.

## Resources and remaining limits

Another 24 native diagnostic runs, separate from ranking, measured maximum
whole-worker peak RSS over three trials per binary at 144 DPI:

| Synthetic case | Before MiB | After MiB |
|---|---:|---:|
| Curves | 28.66 | 28.76 |
| Dense rectangles | 22.47 | 22.36 |
| Embedded TrueType | 21.53 | 21.52 |
| Small JPEG | 34.73 | 34.77 |

There was no material peak-RSS increase in these controls. Warm-loop
process CPU/wall ratios were approximately 0.995–0.998, with no observed
sustained multicore advantage. These are not pure allocation counters or
proofs about all documents.

The real scan exposes a separate resource issue: at 144 DPI its worker peak
was approximately **120.72 MiB for rzpdf versus 67.26 MiB for PDFium/Python**.
Python overhead is included in the latter, so these are process observations,
not engine-only allocation ratios. Image decode/conversion lifetimes and
cache retention need separate profiling. Limits were not raised to hide it.

Remaining priorities are first analytic coverage generation, the initial
CoreText UI-font query, real-image peak memory, fractional output-dimension
policy, wider licensed corpora, and independent browser timing. WASM checks
below verify correctness; they do not rank WASM against PDFium.

## Verification and reproduction

- 108 render library tests and 24 WASM interface tests passed; the encoding
  change passed nine targeted PDF font-encoding tests. The unchanged broader
  PDF suite was not rerun for this small alias change.
- Strict workspace/all-target Clippy passed with default features and with
  `pdf/prepare-diagnostics`; formatting, six harness tests and Python compilation
  passed. All twelve vendored header/license hashes matched their manifest.
- Final pinned release WASM build passed. Direct/Worker regression passed
  56 synthetic plus eight real-file combinations, including cached output,
  clearing and handle state. ICC/Type3 and resource browser regressions passed
  in both transports.
- Real browser text checks retain the existing bounded geometry/coverage
  checks for CoreText/FreeType differences; the scan permits the existing
  browser JPEG budget policy. Neither real case claims native/WASM byte
  identity. Direct/Worker and cold/hit/clear hashes must still match exactly.

```sh
cargo build --locked --release -p render --example benchmark_render
python3 scripts/prepare_real_benchmark_corpus.py output/benchmark-real
python3.11 scripts/benchmark_pdf.py \
  --manifest output/benchmark-real/manifest.json \
  --output output/real-comparison --dpi 72 144 \
  --trials 5 --warmup 2 --repeats 10
```

Local evidence: ignored `output/coretext-collection-20261027/`, including
`engines/`, `real-engines/`, `paired/`, `paired-warm-recheck/`, `font-pair/`,
`n32-cold-pair/`, `paper-pair/`, `resources/`, native validation, before-fix
scan failures, browser reports and build/test logs. Frozen worker SHA-256:

```text
before 84df63d121e77a5a8f5ab413f75bf2ef70d69dc7650996a691962a0c4761c2c2
after  816accae95b447310d496cc918506713da30f2b397414e029f37e8b9f0f7b2e1
```
