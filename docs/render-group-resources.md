# Local transparency targets and resource accounting

The first page of `2608.25512v1.pdf` failed in the browser at 102/108 DPI
(`ICC group conversion peak`) and at 144 DPI (`aggregate group surface bytes`).
At 72 DPI it succeeded. The PDF contains a small isolated ICCBased transparency
Form near the top of the page, not an oversized image. The old compositor
allocated page-sized group buffers and converted/copied the whole raster.

## Implemented correction

Each group now owns a device-space target: global raster origin, local width and
height, and the page-to-target matrix. The renderer saves its parent's target
and restores it on both success and failure. The target intersects the current
device clip with conservative geometric bounds of every PDF clipping operand.
Bezier control hulls are conservative. A two-pixel halo retains the existing
antialias/filter footprint. Original exact clipping is still applied to ink.

Bounds are computed before rasterizing clips. Skia's device_clip_bounds alone
is insufficient: rotated/curved clips can be represented by shaders that do not
shrink those bounds. Canonical clip caches are scoped to the active target;
backing mask images remain in the existing live-image ledger until released.

Nonisolated groups copy just their local initial backdrop, retaining the
backdrop/coverage/shape semantics needed by knockout and nested groups. Group
images and shape images are composited at the correct parent-relative offset.
Soft masks applied to a group are evaluated on the same local grid, with the
frozen parent/group bytes reserved before invoking the mask renderer. Pattern
and mask subrenderers inherit the global raster region and keep tile alignment.

ICC conversion uses the source image extent, preserves unpremultiplied color
and alpha semantics, and transfers the finished allocation into Skia through
the existing release-callback ownership bridge. Nonisolated correction also
transfers native premultiplied N32 bytes. Neither route creates the preceding
extra Data::new_copy of its complete result.

## Budget contract

The browser defaults are unchanged: 4,000,000 output pixels, 16,000,000 render
bytes, 64 MiB logical operation budget, and 256 MiB maximum linear memory.

Within the transparency compositor, max_render_pixels checks individual raster
geometry. max_render_bytes checks simultaneously live raster storage. Group
entry reserves the actual frozen parent and local coverage/shape/backdrop
buffers; nested entry inherits the remaining allowance. Conversion checks the
restored parent plus the actual local source owners and one destination, before
allocation. Shared snapshot storage is not charged as an independent copy;
copy-on-write initial storage and independently corrected images are counted.
Group exit and abort restore the preceding allowance and target.

`ResourceLimitExceeded` reports reason=render_live_bytes, stage=render_group,
group_color_conversion, group_uncompositing, or group_soft_mask, and exact limit
and estimated required live bytes for the failing check. SDK/Worker preserve
these fields. Capabilities expose renderResources.localTransparencyGroups,
groupPixelAccounting, groupByteAccounting and groupBudgetField.

This is scoped raster accounting, not a process-RSS allocator or a complete
migration of every backend allocation to a shared lease pool. Existing image,
font, pattern, general soft-mask and cache policies retain their independent
limits. Their wider consolidation and calibration should use explicit live-byte
instrumentation; this change does not claim that task completed or raise those
ceilings. Native/C-library hidden allocations remain outside logical counters.

The original paper's first page now fits at 144 DPI under the same render
ceilings. A genuinely page-sized ICC group at 144 DPI still exceeds 16 MB and
returns a typed error. Its tiles can render at higher DPI. A whole-page 288 DPI
A4 output still exceeds the 4-million-output-pixel ceiling and is rejected.

## Verification

- Native group/ICC, isolation, knockout, soft-mask and region stitching tests.
- Exact local allocation boundary refusal and clean retry.
- Large-page small/nested/rotated ICC groups at browser budgets, including a
  geometric-bounds regression where Skia uses a shader clip.
- Project-owned generated ICC, nested, rotated, soft-mask, knockout and genuinely
  large group fixtures with pinned hashes; no private document is committed.
- Native and real optimized-WASM synchronous SDK/Worker pixel comparison with
  separately generated MuPDF and PDFium images, plus non-background color and
  transparency probes.
- Tile comparisons, unchanged document revisions, save/Strict reopen, cache
  clear, structured failure/retry, cancellation/new-session recovery, and handle
  release. The original private paper is an optional explicit corpus input.

```sh
cargo test -p render --lib
cargo run -p render --example group_resources -- fixtures OUTPUT_DIRECTORY
cargo run -p render --example group_resources -- render INPUT.pdf OUTPUT_DIRECTORY
output/benchmark-venv/bin/python scripts/masked-preview-references.py \
  pdf/tests/fixtures/group-resources output/group-resources-20261031/references \
  --pattern '*.pdf' --dpis 72 96 144
# After the pinned optimized WASM build:
RZPDF_GROUP_PDF=/absolute/path/to/2608.25512v1.pdf \
  node web/tests/group-resources-browser.mjs
```

Artifacts, original-file render output, build source hashes and acceptance logs
are retained in ignored output/group-resources-20261031. `acceptance.json` records that earlier frozen build and test outcomes. It is not
a current-workspace certificate. Refreshed evidence is under
`output/compatibility-20261031/regressions-final/group/browser-report.json`;
the optimized build is pinned by `wasm-build-final.json` in that same topic root.

## Reference implementations

The design follows local-target allocation, not a borrowed numeric cap:
[MuPDF fz_draw_begin_group](https://github.com/ArtifexSoftware/mupdf/blob/master/source/fitz/draw-device.c)
transforms/intersects group bounds before creating a pixmap;
[PDFium ProcessTransparency](https://github.com/chromium/pdfium/blob/main/core/fpdfapi/render/cpdf_renderstatus.cpp)
intersects object bounds with device clipping and translates the child origin;
[PDF.js beginGroup](https://github.com/mozilla/pdf.js/blob/master/src/display/canvas.js)
uses a local canvas and can avoid intermediate canvases under guarded conditions.
Their implementation details are not evidence of a universal total-memory cap.


Earlier frozen native acceptance: 118 renderer tests and 25 web-wasm host tests passed. The
private first page passed 72/96/102/108/144 DPI with max_render_bytes=16000000
and max_render_pixels=4000000 explicitly set on both Document and SkiaRender;
its 288 DPI whole-page output remains an expected geometry refusal. Optimized
WASM synchronous SDK and Worker both passed the private-page 102/108/144 DPI
checks, synthetic reference/oracle tests, tiles and typed failure recovery.
The browser report also records synchronous-module linear-memory capacity per
fixture; that observable is not live bytes or process RSS. Final build identity
and legacy browser regressions for that frozen build are recorded in acceptance.json.

The compatibility-program refresh passed 118 renderer and 27 host-interface
tests. Its final optimized module passed both transports, the six fixtures and
the original first-page 72/102/108/144 DPI cases. See the
[compatibility evidence](compatibility-program.md) for current build identities
and the separate broader-corpus coverage.
