# Render session architecture and performance

This round keeps PDF interpretation independent of Skia and separates reusable
backend resources from request-local surfaces. It builds on the
[second optimization round](render-performance-round2.md).

## Implementation and acceptance scope

- [x] Introduce `RenderSession` for bounded backend resources. A new raster
  target can reuse resources without retaining another target or a document.
- [x] Reuse embedded typefaces, converted non-stencil images, normalized path
  outlines, and solid SrcOver canonical coverage rectangles.
- [x] Add borrowed `Device` replay entrypoints. Existing owned-event devices
  continue to work through default implementations; `PaintingDevice` preserves
  nonpainting colorants and text mode filtering. The renderer borrows common
  image, path and text events instead of copying editing provenance per replay.
- [x] Integrate sessions into WASM requests, cache statistics, explicit cache
  clearing, document release and module disposal.
- [x] Verify rendering equivalence, independent target lifetimes, DPI/region
  changes, output profile/shape variants, document/revision/font epoch changes,
  optional cache exhaustion, and warm-cache resource-limit rejection.
- [x] Complete serial native timing, separate resource diagnostics and real
  browser direct/Worker acceptance; results follow below.

## Ownership and invalidation

```text
Document / revision → PreparedPage (immutable interpreted scene)
                                 ↓ borrowed replay
RenderSession → backend fonts / images / geometry
                                 ↓ shared immutable resources
RenderRequest → temporary SkiaRender surface → caller-owned RGBA
```

The existing `PreparedPage` remains the scene representation. This change does
not introduce a second full editing object graph. The main replay events can
borrow its arrays and metadata. Complex text mode changes and per-glyph knockout
still copy the events that they need to modify.

Native consumers can explicitly retain a session:

```rust,ignore
let session = RenderSession::new(document.resource_limits());
let mut target = session.renderer(&document, font_epoch, dpi, dpi)?;
target.set_region(region)?;
prepared.replay(&document, &mut target)?;
let output = target.to_rgba()?;
// Drop target; session remains available for the next request.
```

The session invalidates on document identity, revision or fallback font epoch
changes. Image keys verify weak source identity, device linear transform, shape
mode, output ICC profile, rendering intent and ICC fallback policy. Translation
and clipping are applied at draw time. Stencil images retain their dynamic fill
and pattern behavior and are not retained in the image cache.

Path keys include actual geometry and fill rules, with full equality checking
behind the hash. Canonical coverage keys additionally include antialiasing and
the visible/sample bounds. Color, alpha and current clipping are applied when
replaying the cached rectangles. Changing output color or alpha does not reuse
paint state. Normalized geometry can be reused across tiles; coverage reuse is
currently scoped to matching bounds, rather than arbitrary overlapping tiles.

## Memory and correctness boundaries

The cache uses one byte budget and LRU ordering across resource kinds. The
estimated storage ceiling is the smaller of 16 MiB and half the document decode
budget: 8 MiB for the default browser profile, 4 MiB for the conservative profile.
Native callers can reduce it further with `with_cache_limit`; zero disables
retention. At most 4096 entries are kept. Oversized entries and failed optional
reservations fall back to rendering without retention.

Estimates charge path keys, font program copies, image storage (including a
conservative mipmap allowance), coverage vector capacities and entry overhead.
They are not exact Skia allocation counters or RSS. The browser retains backend
resources for only the currently rendered document, so multiple open documents
do not multiply the module's optional cache allowance.

Cache hits preserve the original geometry work charges and scratch/mipmap
preflight. They do not enlarge image decode, render surface, operation or WASM
linear-memory limits. Nested pattern and soft-mask targets share immutable
resources but keep their own surfaces and reduced temporary budgets.

`cacheStatistics()` and `clearRenderCaches()` expose additional
`renderSession*` counters through both the direct SDK and Worker. Hit/miss and
eviction counters are cumulative; entry counts and estimated retained bytes
reflect current contents. Clearing caches or closing documents does not invalidate
caller-owned output pixels and does not promise that WASM linear memory shrinks.

## Measurement

Formal native timing keeps the previous document-open, prepare, replay and owned
RGBA boundaries, with diagnostics disabled. A supplemental `fresh_renderer`
JSON worker option creates a request-local target for every warm iteration,
including target construction in the warm timer, while retaining the session.
This checks whether benefits survive the browser-style target lifecycle.

Browser regression renders each case cold, again through the retained session,
and after clearing caches. It requires identical output within each module and
checks resource reuse and clearing through both SDK transports. This is not a
WASM-versus-PDFium timing comparison.

## Measured results (2026-10-26)

Formal results are in `output/session-optimization/native-comparison/`. They use
exactly the previous 14 files (SHA-256 checked), 72/144 DPI, five independent
process trials, two warmups and ten warm replays. No builds, browser checks or
resource diagnostics ran concurrently with formal timing. Python 3.11.5,
PyMuPDF 1.26.7, PDFium 145.0.7616.0 and Pillow 12.2.0 match the previous run.

All **420 trials succeeded**. The unchanged three-engine image gate passes
**22/28 combinations**; two blank baselines are excluded from speed ratios.
All **44 selected page outputs** have identical dimensions and RGBA bytes to
the previous round. Existing six image-gate exclusions remain excluded.

| Operation | rzpdf / PDFium | rzpdf / MuPDF | Paired combinations |
|---|---:|---:|---:|
| First page after renderer initialization | 3.70x | 2.66x | 20 |
| Retained scene warm replay, including owned RGBA | 0.78x | 1.24x | 20 |
| Subsequent pages | 1.74x | 2.48x | 4 |

Ratios are geometric means of paired latency ratios, not a promise for arbitrary
PDFs. Warm performance reaches the desired overall range on these synthetic
samples; **first-page latency remains substantially behind PDFium**.

| Warm replay, ms | Previous rzpdf | Current rzpdf | Current PDFium |
|---|---:|---:|---:|
| Curves, 72 DPI | 57.26 | 21.11 | 8.03 |
| Curves, 144 DPI | 109.76 | 45.21 | 19.64 |
| Rectangles, 144 DPI | 38.37 | 17.44 | 16.38 |
| Small JPEG, 144 DPI | 49.28 | 12.03 | 40.28 |
| Flate image, 72 DPI | 40.09 | 2.30 | 10.43 |
| TrueType text, 144 DPI | 5.00 | 5.63 | 4.28 |

Compared with the historical run, rzpdf warm latency's geometric mean falls
39.9%, while first-page latency rises 6.9% and subsequent-page latency rises
10.6%. These are separate runs, not an interleaved before/after experiment:
other engines also slowed on several text samples (e.g. dense Latin text at
144 DPI: MuPDF warm +26%, PDFium +18%). Therefore the entire change cannot be
attributed to cache implementation. Establishing caches adds work to cold
rendering; this round does **not** claim a first-page improvement or absence
of per-case regressions. Use the current same-run engine ratios for rankings.

A separate native experiment alternated retained versus fresh raster targets,
three trials per case, keeping the scene and session alive. It confirms that
benefits survive target destruction rather than depending on a retained surface:

| 144 DPI, warm ms | Retained target | Fresh target with retained session |
|---|---:|---:|
| Curves | 43.50 | 45.23 |
| Rectangles | 16.90 | 16.15 |
| Small JPEG | 10.72 | 12.28 |
| TrueType text | 4.60 | 6.15 |

These supplemental timings are not mixed into the formal engine ranking.
Request-local target construction remains measurable, particularly for small
text workloads. Raw results: `output/session-optimization/fresh-targets.json`.

## Memory and browser evidence

Separate diagnostics cover four cases at 144 DPI, three trials per engine
(**36 successful trials**). CPU/wall ratios remain near one; there is no evidence
of a sustained multicore advantage in these measurements. rzpdf phase RSS:

| Case | After first output, MiB | After replay, MiB | Maximum worker peak, MiB |
|---|---:|---:|---:|
| Curves | 28.00 | 28.42 | 28.76 |
| Rectangles | 21.72 | 22.23 | 22.48 |
| Small JPEG | 34.62 | 34.69 | 34.79 |
| TrueType text | 24.73 | 24.80 | 25.00 |

Phase values are medians; peaks are maxima across trials. The earlier small-JPEG
peak was 34.77 MiB and curve peak 34.40 MiB in the separate diagnostic run.
These measurements do not imply that caching consumes no memory: allocator
reuse, temporary lifetimes and backend allocations affect RSS. Python-adapter
RSS is not directly comparable to pure engine allocation, and native RSS is not
WASM linear-memory usage. Diagnostics: `output/session-optimization/resource-diagnostics/`.

The optimized pinned WASM build passes browser direct and real Worker checks
for **56 case/DPI/transport combinations**, with three renders per combination:
cold, session hit and rebuilt after clear. Each module's three outputs are
identical; direct and Worker hashes also match. Font and image hits and zero
entries after clear are explicitly checked. Existing ICC/unsupported-group
recovery, sparse CID, cyclic color, Type3, high-DPI tiles, reduced JPEG decode
and structured resource errors pass through both transports.

A supplemental Worker trace compares the frozen pre-change WASM against the new
module on the small JPEG: seven region requests with panning and DPI changes.
All seven output buffers match exactly. At repeated 72-DPI regions, illustrative
latencies change from roughly 106–108 ms to 19–21 ms; cold and changed-DPI requests
remain around the previous range. This is one trace, not a statistical browser
benchmark. It also exposes the intentionally conservative transform-based image
key: changing DPI may rebuild resources. Closing the document leaves zero tracked
handles and caller-retained buffer bytes in both modules.

Browser evidence: `output/webassembly/render-benchmark-regression/report.json`,
`output/webassembly/render-cache/report.json`, `output/webassembly/icc-browser/report.json`
and `output/webassembly/resource-browser/report.json`. The frozen pre-change WASM
hash is `432eb1bbbdd2c016eba9c8530c70c294d8e90f34671b91a03d9d8e644936a044`;
new hash is `5662f654660cd3d0198b90721f0ba856b71d921634510fc2c276dbcf600c46c4`.

Validation: render library **102 tests**, PDF replay **6 targeted tests**, WASM
host **24 tests**, benchmark Python **6 tests**, strict workspace/all-target
Clippy, TypeScript build, formatting and diff checks. The full PDF workspace
integration suite was not rerun.

## Remaining optimization boundaries

First-page font/backend initialization and cold-page preparation remain work for
a separate measured change. Curves are faster but still around 2–3x PDFium in
these samples because cached coverage still needs many paint submissions.
Large images exceeding the cache budget render without retention; repeated
large-scan conversion is not solved by this cache. Wider reuse across different
DPI or overlapping tile coverage is possible but needs keys and budget accounting
that preserve masks, interpolation and canonical sampling. No unconditional
backend-path shortcut or relaxed quality/resource threshold was introduced.
