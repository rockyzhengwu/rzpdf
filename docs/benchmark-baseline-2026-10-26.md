# Native rendering baseline: 2026-10-26

**Correction:** the initial MuPDF adapter used PyMuPDF's `Pixmap(cs, rect, alpha)`
constructor, which creates a non-null separation set in the installed binding.
An independent solid DeviceRGB oracle showed that this enabled an unintended
conversion path. The adapter now uses the null separation set of the standard
`DisplayList.get_pixmap` implementation, with a regression test. **MuPDF timings,
ratios and three-way quality selection in this historical record are superseded
by the corrected optimization run.** Original rzpdf/PDFium raw timings are
unchanged, but their aggregate here still describes the original selected subset.

This is an initial synthetic baseline, not a general ranking of PDF engines.
See [the benchmark method and reproduction commands](benchmarking.md).

## Environment and coverage

- macOS 13.7.8; Intel Core i7-7820HQ 2.90 GHz; 8 logical CPUs; 16 GiB RAM.
- rzpdf native optimized Release build, starting revision `88dddab` plus the
  benchmark tooling; Skia backend (`skia-safe` 0.93.1).
- MuPDF **1.26.12**, accessed through PyMuPDF **1.26.7**.
- PDFium **145.0.7616.0**, accessed through pypdfium2 **5.3.0**.
- Python 3.11.5; Pillow 12.2.0. Engine imports/process startup excluded from
  operation timers; owned RGBA copies included. CPU rendering, normal caches,
  compatible parsing, default native resource limits, no annotations/widgets.
- 14 generated cases × 72/144 DPI = 28 case/DPI combinations; 3 fresh-process
  trials each, with 2 warmups and 10 measured replays per trial.
- Two serial sessions: the original 10 cases, followed by four TrueType/Chinese
  cases. Regeneration produced identical hashes for the original ten inputs.
  This was a local development machine, not a controlled performance lab.

There were **252 timing trials: 231 successful and 21 errors**. The errors were
18 Type1 font failures and three curve coverage resource failures. No worker
trap or timeout occurred. All other exclusions were image agreement failures.

**16/28 combinations passed three-way image agreement**, including two blank
baselines. The aggregate below uses the **14 passing nonblank combinations**.
Failed and differing outputs remain visible in the detailed report.

## Paired aggregate

Each number is the geometric mean of per-case median rzpdf/reference elapsed
time ratios. **Greater than one means rzpdf takes longer.** Blank baselines are
excluded. These are descriptive figures from a small selected subset.

| Operation | rzpdf / MuPDF | rzpdf / PDFium | Case/DPI pairs |
|---|---:|---:|---:|
| First page in a new document | 1.03× | 4.73× | 14 |
| Explicitly retained page replay | 1.06× | 3.08× | 14 |
| Next page in the same document | 0.45× | 2.17× | 2 |

The next-page result contains **only the Chinese multipage input at two DPIs**.
It cannot establish a general sequential-reading advantage. Results vary
substantially by content, so the aggregate alone is a poor optimization guide.

## Selected 144 DPI cases

Times are milliseconds, medians across three trials; warm replay pools 30
observations. These rows passed the quality gate.

| Case / operation | rzpdf | MuPDF | PDFium |
|---|---:|---:|---:|
| Embedded TrueType Latin, first page | 62.74 | 81.13 | 12.95 |
| Embedded TrueType Latin, warm replay | 11.54 | 30.82 | 4.89 |
| Sparse Chinese, first page | 54.07 | 79.73 | 13.04 |
| Sparse Chinese, warm replay | 10.53 | 33.20 | 4.81 |
| Chinese multipage, next page | 13.32 | 35.02 | 6.65 |
| Small JPEG, first page | 192.76 | 119.76 | 58.75 |
| Small JPEG, warm replay | 154.91 | 47.00 | 40.30 |
| Overlapping transparency, first page | 25.47 | 86.81 | 44.06 |
| Overlapping transparency, warm replay | 12.15 | 35.61 | 36.37 |

For the small JPEG at 144 DPI, whole-worker peak RSS was 34.78 MiB for rzpdf,
67.82 MiB for MuPDF and 41.43 MiB for PDFium. Python adapter/runtime memory is
included for the latter two; these figures do not establish engine-only memory
efficiency or predict browser WASM memory.

## Findings to investigate

1. **Embedded Type1 compatibility:** sparse, dense and multipage PDFs generated
   with the repository's `n019003l.pfb` fail in rzpdf with
   `font 'Nimbus Sans L Regular' has no drawable glyph for character code 49`.
   Both independent engines render them. Preserve these cases as compatibility
   reproducers; do not substitute a different font and erase the failures.
2. **Curve resource accounting:** the 1,000-curve input at 144 DPI returns
   `resource limit exceeded: canonical curve coverage work`. Review the
   rendering work and accounting before considering any budget change.
3. **Image rendering cost:** the passing small JPEG requires 154.91 ms for
   warm replay, versus 47.00/40.30 ms. Profile image resampling, color conversion
   and surface copies separately before optimizing.
4. **Output differences:** large scan images fail the RGB error gate at both
   DPIs. Dense Chinese also fails at both DPIs, including MuPDF versus PDFium;
   their ink geometry agrees but coverage/weight differs. Rectangles at 72 DPI
   fail RGB error against MuPDF. These are differences to inspect, not proof
   of incorrect PDF semantics. Thresholds were not relaxed after seeing them.

For illustration only, the excluded 8.75-megapixel scan at 144 DPI takes
1,024.93/163.27/161.27 ms for first page and 903.16/62.49/56.44 ms for warm
replay (rzpdf/MuPDF/PDFium). **These are not equivalent-output speed ratios**
and are absent from the aggregate.

Representative text, vector, transparency and flagged image outputs were
visually reviewed using contact sheets. This does not replace full-size visual
review or a broader production corpus.

## Local artifacts

The raw generated data is intentionally ignored by Git:

- `output/benchmark-20261026/`: first ten cases, raw trials, PNG/RGBA and stderr.
- `output/benchmark-fonts-20261026/`: four additional font cases.
- `output/benchmark-combined-20261026/`: combined `report.md`, `summary.json`,
  `raw.jsonl`, `quality.json`, `sources.json` and a contact sheet. The final
  opaque-output gate was reapplied to stored RGBA; timers were not rerun.

The initial worker recorded PyMuPDF's binding version as its `version` field.
The actual MuPDF core version above was checked from `fitz.VersionFitz` in the
same environment. Subsequent tool runs record both engine and binding versions.
The combined raw records also annotate blank baseline trials explicitly; timing
values are unchanged. A final one-case smoke run verifies the revised reporting
and worker version fields and is excluded from this baseline's statistics.
