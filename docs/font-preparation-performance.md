# Font preparation and backend reuse

Measured on macOS x86_64 on 2026-10-26, against committed code `2771736`.
This round reduces predefined CMap construction and repeated font-manager
initialization. It does not switch raster backends or raise resource limits.

## Changes and attribution

`Identity-H` and `Identity-V` now represent their 65,536 CID mappings
arithmetically instead of allocating a hash-map entry for each code. Explicit
overrides, code lengths, ambiguous lengthless lookup, inheritance precedence,
WMode and the old source/depth/logical-entry limits remain covered by tests.
The separately budgeted parser for embedded CMap programs remains unchanged.

Embedded typefaces share a lazily created font manager through `RenderSession`.
The manager participates in the existing LRU, clearing and document/revision
invalidation. Its accounting allowance is 32 KiB plus entry overhead; this is
an estimate for an opaque backend object, not an RSS bound. Font retention has
both the session byte budget and a 128-face ceiling. Zero cache capacity retains
neither manager nor faces. Browser statistics expose manager entries and hits.

The separate `profile_font_backend` example attributed the initial macOS cost:
manager construction 33.66 ms, loading one embedded face 1.34 ms, first draw
3.16 ms. Subsequent manager constructions still took approximately 16–21 ms.
A thread sample places the manager construction in CoreText family enumeration
and synchronous system IPC. These are wall-time samples, including waits, not
a claim that enumeration consumes that proportion of CPU time.

Skia's custom empty manager is used when available. It is unavailable in this
macOS build, and `FontMgr::empty()` cannot load the specimen font. The supported
system backend therefore remains the fallback. Its first initialization stays
inside the first-render timer; this round removes repeated initialization for
multiple faces, without moving the first cost outside measurement.

## Paired before/after measurements

Five interleaved trials per binary, case and DPI produced 300 successful runs:
the original 14 cases at 72/144 DPI plus one supplementary three-font case.
All 46 selected page outputs were byte-identical. The supplement is excluded
from the original corpus aggregate and from the three-engine ranking.

Across the original 26 nonblank combinations, geometric mean after/before:

| Phase | Ratio | Change |
|---|---:|---:|
| Page preparation | 0.487 | 51.3% less time |
| First output | 0.922 | 7.8% less time |
| First raster phase | 1.001 | Essentially unchanged |

On the 20 nonblank combinations eligible for cross-engine ranking, preparation
took 54.1% less time and first output 8.1% less time.

Selected 144 DPI medians, in milliseconds:

| Case | Prepare before | Prepare after | First before | First after |
|---|---:|---:|---:|---:|
| Dense Latin text | 13.24 | 6.43 | 68.79 | 59.40 |
| Embedded TrueType | 21.95 | 3.36 | 110.55 | 96.11 |
| Dense CJK | 15.47 | 5.59 | 94.98 | 83.85 |
| Three-font supplement | 38.71 | 4.44 | 152.22 | 58.84 |

The supplementary multi-font case reduced first-output time by 61.3% at
144 DPI and 62.2% at 72 DPI. It contains three distinct embedded font programs
with checked glyph coverage; it is evidence for repeated initialization, not
a prediction for every document.

One five-trial result suggested an 18.4% regression for the small JPEG at
144 DPI. A separate 20-trial interleaved recheck at both DPIs (80 runs) did not
reproduce that magnitude: first-output ratios were 1.018 at 144 DPI and 1.029
at 72 DPI, with identical pixels. There are no image-path changes this round.
User applications remained active, so absolute timings fluctuate; no CPU
affinity or fixed-power-profile control is claimed.

## Fresh comparison with PDFium and MuPDF

The original 14-case corpus, two DPIs, five trials, two warmups and ten warm
replays produced 420 successful trials. All 44 selected native outputs match
the previous committed run byte-for-byte. The existing image agreement gate
remains 22/28; its six failures were not relaxed or included in rankings.
Excluding two blank combinations leaves 20 common combinations. Ratios are
geometric means of rzpdf time divided by the corresponding engine time:

| Phase | PDFium | MuPDF |
|---|---:|---:|
| First output | 2.99× | 2.13× |
| Warm replay | 0.80× | 1.23× |
| Next page | 1.30× | 1.87× |

Next-page evidence contains only four paired combinations. The previous
committed PDFium comparison was 3.27× / 0.79× / 1.33× respectively. Separate
cross-engine runs support the trend; the interleaved paired experiment is the
stronger evidence for the isolated code change.

First output includes opening, page preparation, first raster and owned RGBA
readback. Warm replay reuses prepared pages and session resources. Next-page
timing uses an already open document. None of these measures WASM performance.
RSS in the generated report includes each entire adapter process, including
the Python runtime for PDFium/MuPDF; it is not pure engine allocation.

Versions: PDFium 145.0.7616.0 / pypdfium2 5.3.0; MuPDF 1.26.12 /
PyMuPDF 1.26.7. See [benchmark methodology](benchmarking.md).

## Verification and reproduction

- Workspace library tests: 987 passed, one existing ignored test.
- Strict workspace/all-target Clippy passed with default features and with
  `pdf/prepare-diagnostics`; formatting and six benchmark harness tests passed.
- TypeScript build and pinned release WASM build passed.
- Browser direct/Worker benchmark regression: 56 combinations passed,
  including manager cache reuse and clearing.
- ICC/Type3 and resource browser regressions passed in both transports.

The default corpus remains 14 cases. Generate the optional specimen with:

```sh
python3 scripts/prepare_benchmark_corpus.py output/multifont --multifont-only
cargo run --release -p render --example profile_font_backend -- \
  pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf 5
```

Local evidence is in `output/font-backend-20261026/`: `paired-final/`,
`jpeg-recheck/`, `engines/{raw.jsonl,quality.json,summary.json,report.md}`,
`font-manager.sample.txt`, build/test logs and browser logs. Artifacts are
ignored rather than committed. Binary SHA-256:

```text
before b99d4ab22d59f62cd5ba5ba555e105dacb0d000a028d1ec6fe46cca324829187
after  72c7c5c2cf0a8249eb98cb42762c30a0688df70c53a1a6624d10fd3817e27733
```

Remaining priorities are eliminating the first macOS backend enumeration
through a supported backend API, improving complex-curve raster cost, and
expanding licensed real-document and independent browser timing coverage.
