# Page preparation: diagnostics and optimization

This follows the [render-session work](render-session-optimization.md). The goal
is to improve cold page preparation without dropping Unicode, edit provenance,
resource checks or rendering correctness.

## Scope

- [x] Add explicitly enabled, exclusive preparation phase timings.
- [x] Identify the dominant work before changing text interpretation.
- [x] Preserve buffered input across small lexer seeks and avoid ordinary
  source-position queries by tracking reader position.
- [x] Reuse the content parser's classification token during object parsing.
- [x] Finish interleaved before/after measurements and native/browser acceptance.

## What the diagnostic phases mean

For standalone diagnosis, build and run the separate worker:

```sh
cargo build --release --locked -p render --example profile_prepare --features pdf/prepare-diagnostics
printf '%s\n' '{"path":"output/benchmark-corpus-v2/text-dense.pdf","input_mode":"file","dpi":144,"repeats":1}' \
  | target/release/examples/profile_prepare
```

It runs no raster rendering. Each repeat clears derived caches; later repeats
still reuse source objects in the same document. For a fresh-document profile,
start a new process with one repeat. A `memory` input mode is also available.
The existing `benchmark_render` JSON worker additionally supports `"profile_prepare": true`
when built with `--features pdf/prepare-diagnostics`. `preparation_profile` reports time
and call counts for page setup, content streams, operator parsing, font selection,
text operators, XObjects, display collection, other operators and cache accounting.
Nested phases are exclusive, including child Form interpretation. This is a
partition by execution stage/operator category, not a promise that every font
operation in any PDF is performed by `Tf`. For example, ExtGState may also set a
font. XObjects include images and Forms; inline payload parsing begins in the
operator parsing phase.

The feature is disabled by default, including WASM builds. Default preparation
has no diagnostic clock reads. Profiling is also opt-in when the feature is
compiled in. Failed and unwound requests clear the recorder. Cached pages remain
cached; clear the document's render caches to measure new interpretation. Source
object caches and OS file caches are separate, so reopening a document is needed
for a fresh-document comparison. Diagnostic timings are not used for rankings.

The JSON worker also accepts `"input_mode": "memory"` to open an immutable byte
source, or `"file"` (the default) to use the normal file API. It includes the file
read inside the open timer for memory mode. These modes have different ownership
semantics and should be compared deliberately.

## Findings and implementation

The initial diagnostic run showed dense Latin text preparation around 29 ms:
font selection/resource loading occupied about 22 ms, text operators about 1 ms,
and display collection less than 0.1 ms. The font category includes loading its
source PDF objects; it is not entirely font-program CPU work. Repeating after
clearing derived caches was much faster because source objects remained loaded.
Curves were dominated by operator parsing, and image pages by XObject handling.

Two redundant operations were confirmed in the actual code path:

1. `StreamReader::seek` always performed an absolute seek. The lexer frequently
   rewinds a delimiter or peeks ahead. Absolute `BufReader` seeks discard buffered
   input; subsequent byte reads repeatedly refill it. Small relative seeks can
   remain inside that buffer. Tracking the exclusively owned reader's current
   position also avoids an underlying position query for every rewind. On partial
   I/O failure the tracked position becomes unknown, and recovery resynchronizes
   it. Outside-input/large offsets and unsupported relative operations retain an
   absolute fallback. Regression coverage checks buffered refills and seek calls,
   partial failures, EOF, arbitrary readers and parser recovery.
2. The content parser lexed each operand to classify it, discarded that token,
   then rewound and lexed it again as an object. It now transfers the existing
   token into the same object parser, retaining numeric lexemes, reference
   lookahead, compound objects and the depth guard. The separately budgeted path
   keeps its existing token lease ownership and preflight order.

Unicode, character layout, graphics-state snapshots and editing provenance remain
intact. The measurements did not justify removing them or changing their public
representation in this round.

## Reproducing a paired comparison

Save the old Release worker before modifying/building the implementation, then
build the new worker **without** `prepare-diagnostics`:

```sh
cargo build --release --locked -p render --example benchmark_render
python3 scripts/compare_native_preparation.py \
  --before /path/to/saved/benchmark_render \
  --after target/release/examples/benchmark_render \
  --manifest output/benchmark-corpus-v2/manifest.json \
  --output output/preparation-comparison --trials 5
```

The output directory must be new. The driver interleaves before/after workers
with deterministic randomized order, verifies corpus hashes and checks all
selected pages' dimensions and RGBA bytes. It records binary hashes, raw results
and per-case medians. Each worker opens a fresh document. The first-page timing
includes document open, prepare, raster and owned RGBA; process initialization and
artifact writes remain outside operation timers. Subsequent selected pages are
also rendered for correctness but do not enter `first_ms`. This comparison does
not itself rank PDFium or enforce a cross-engine quality gate.

## Final paired results (2026-10-26)

`output/page-preparation-20261026/paired-final/` records five independent trials
per binary for the same 14 files at 72 and 144 DPI: **280 successful runs**.
Before/after order is interleaved using a fixed random seed, diagnostics and warm
replays are disabled, and each process opens a fresh document. No builds or tests
ran concurrently with these measurements. The old worker was frozen before any
changes in this round. All **44 selected page outputs** have matching dimensions
and identical RGBA bytes.

Across the **26 nonblank file/DPI combinations**, the geometric mean of the
paired after/before ratios is:

| Operation | After / before | Observed latency reduction |
|---|---:|---:|
| Document opening | 0.361 | 63.9% |
| Page preparation | 0.702 | 29.8% |
| First page, including owned RGBA | 0.885 | 11.5% |
| First raster and readback | 1.007 | Essentially unchanged |

For the same **20 nonblank combinations** eligible in the previous three-engine
comparison, preparation falls 31.0% and first-page latency falls 12.1%. This is a
before/after comparison of rzpdf, not a new PDFium/MuPDF ranking. It does not
update the historical 3.70x first-page ratio by extrapolation.

Representative 144-DPI medians, in milliseconds:

| Input | Preparation before | Preparation after | First page before | First page after |
|---|---:|---:|---:|---:|
| Dense Latin text | 25.43 | 12.04 | 71.80 | 57.51 |
| Multipage Latin text, first page | 22.96 | 9.60 | 69.67 | 52.79 |
| Dense Chinese text | 14.88 | 9.17 | 61.14 | 53.84 |
| TrueType text | 9.02 | 7.37 | 50.62 | 47.83 |
| Curves | 6.18 | 6.01 | 135.00 | 136.54 |
| Large JPEG scan | 63.69 | 64.37 | 372.08 | 375.38 |

Do not generalize the text-page improvement to all page types. Curves and large
image pages are essentially unchanged in this run. The only preparation median
more than 10% slower is the blank page at 144 DPI, 0.083→0.092 ms; no first-page
median regresses more than 10%. Relative percentages on such tiny measurements
are particularly sensitive to noise.

Separate final diagnostics (`profile-final.json`) show file and memory input
preparation stages converging after buffering is preserved. Dense Latin font
selection/resource loading is about 7.98 ms for file input and 7.81 ms for byte
input; text operators remain about 1 ms and collection about 0.04 ms. Font setup
still dominates that sample. Curves spend about 4.5 ms in operator parsing, and
the small JPEG about 7 ms in XObject handling. These diagnostic measurements
include instrumentation overhead and are not mixed into the formal ratios.

## Validation and boundaries

- Final `cargo test --locked --workspace --lib`: **982 passed, 1 existing ignored**
  (PDF 855, renderer 102, WASM host 24, font data 1).
- Two profiling-specific tests pass with `prepare-diagnostics`, including cached
  snapshots and recorder recovery after an error or panic.
- Strict workspace/all-target Clippy passes with the feature disabled and enabled.
- The pinned optimized WASM build passes. Real browser direct and Worker checks
  pass **56 case/DPI/transport combinations**, including repeated rendering and
  rebuilding after cache clear. Existing ICC/unsupported recovery, sparse CID,
  cyclic color, Type3, high-DPI region, reduced image decode and structured error
  checks also pass through both transports.
- The Python benchmark regression suite passes six tests. Formatting, Python
  compilation and diff checks pass. This does not claim all workspace integration
  tests were rerun.

The primary measured gain is for the native **file-backed** path. Browser byte
input already avoids filesystem refills; this round establishes browser
correctness, not an equivalent browser speedup. No new cache or resource limit
increase is involved. Unicode, text positions, editing source ranges, PDF number
lexemes, indirect-reference lookahead and depth checks are retained.

Remaining work includes font setup (including measuring predefined CMap expansion
and font-program decoding separately), numeric-heavy content parsing, and image
resource decoding. First-raster font backend initialization remains separate from
page preparation. Avoid changing these costs together without another controlled
measurement.

Before worker SHA-256: `532c961e257a3ea8b43b258c36f41bd33ea51c5cec11accdf6ab8bd0e0a2a71e`.
After worker SHA-256: `b99d4ab22d59f62cd5ba5ba555e105dacb0d000a028d1ec6fe46cca324829187`.
Final WASM SHA-256: `23884ee5090ee5f272dff810ecf2a9f18e5c5fce1873bd80dab3a318bdaefb33`.
