# Browser rendering resources: implementation and acceptance

Scope: improve high-zoom tiled rendering and bounded scan-image rendering,
without changing extraction/save fidelity or claiming a process-wide memory cap.
Environment installation was explicitly authorized on 2026-10-25 after the
initial prerequisite block. Use project-local pinned tools, without changing
system security, default Rust/Node selection or dependency lockfiles. No larger
Wasm memory, fuzzing, or automatic commit/push.

Status (2026-10-25): WR-01–07 are complete within the explicit limits below.
After user-authorized environment setup, the pinned optimized Wasm build and
the real Chromium checks passed through both synchronous and Worker transports.
This closes the resource slice, not WA-11/12, a full browser release, all codecs,
all platforms or a process-memory sandbox. API/usage: [resource policy](resources.md).

## Work packages

- [x] WR-01: expose engine capabilities before open, effective document limits,
  and validated `default`/`conservative` resource profiles chosen at open.
- [x] WR-02: typed resource failures at image/raster limits with stable reason,
  stage, limit and actual fields, preserved through native ABI, SDK and Worker.
  Legacy unclassified failures remain explicit, never classified by message text.
- [x] WR-03: document render-cache statistics and cache release via SDK/Worker;
  distinguish estimated storage, active outputs and module-level Skia caches.
- [x] WR-04: region DPI up to 1152 (full-page stays 288), while retaining
  1024 tile side, 32768 logical raster side, output pixels/bytes and finite checks.
  Synchronize native, SDK, Worker, raster helper and conformance tests.
- [x] WR-05: render-only target-resolution JPEG decoding; retain bounded source
  dimensions, select codec-native reduced decoding before output allocation,
  account for transforms, revision/scale cache identity and original save bytes.
  Extraction and editing do not inherit a rendering approximation. Preserve
  full-decode pixels when already fitting; reduce only oversized sequential,
  unmasked single-stage JPEGs. Progressive/Type3/pattern paths are not relaxed.
- [x] WR-06: evaluate JPX and masked images separately. Unsupported reduced
  paths retain bounded full-resolution decoding or fail explicitly; never resize
  an already decoded full image and call that reduced-memory decoding.
  Evaluation: JPX has a native resolution hint, but the current full-source
  scratch/code-block envelope is not proven reducible. Mask/Matte reduction
  needs coordinated sampling and unblending. Both advertise reduction=false.
- [x] WR-07: deterministic native/SDK/Worker tests, real Wasm build/browser
  verification where the installed toolchain permits, docs and acceptance record.

## Acceptance

1. Query capabilities without a document. Invalid profiles reject before open;
   effective limits are immutable and never exceed build ceilings.
2. A 576/1152 DPI tile succeeds within geometry budgets; oversized/full-page,
   nonfinite, out-of-page and above-ceiling requests fail consistently at each layer.
3. A generated JPEG scan larger than four million source pixels can render at a
   small target without a full source RGB/RGBA allocation. Larger target requests
   still reject when decode budgets cannot be met; transforms and repeated draws
   cannot reuse an undersized cached result. Save/extract keep original semantics.
4. Masks/Matte, colours and unsupported codecs do not silently lose fidelity.
   Record supported reduced paths and unavoidable codec scratch separately.
5. Structured error metadata survives Worker round trips. Pixel-limit failures do
   not imply retrying smaller tiles fixes image decoding. Unknown allocation sizes
   are absent, not invented. Fatal traps still invalidate the session.
6. Cache hits are observable, release drops cache-owned entries, retained caller
   buffers stay owned, and repeated clear/re-render works without invalidating edits.
7. Record exact executed tests and platform/toolchain limitations; no full-suite,
   peak-memory or browser claim without an executed check.

## Local verification record — 2026-10-25

Base: `b7c6cb2`; changes remain uncommitted. macOS x86_64, Rust 1.99.1.

| Check | Executed result |
|---|---|
| web-wasm native ABI suite | 23 passed |
| PDF image unit tests | 25 passed; the existing optional private-file test remains ignored |
| render_image_resolution | 4 passed; includes 8.75M source pixels, 16M source ceiling, decoded-byte rejection, transform/cache separation, progressive/mask fallback, original-save bytes and display metadata |
| prepared_page / resources / image_extraction | 7 / 14 / 9 passed |
| JPEG codec units | 3 passed |
| Native region/stitch regression | 11 passed |
| SDK / Worker / viewport / text scripts | all four Node scripts passed; the new SDK/Worker script uses stubs |
| TypeScript | installed 5.9.3 compiler passed; reused a local compiler, no npm installation |
| Build/prerequisite verifier tests | 9 Python tests passed |
| Format, diff, browser-driver syntax | passed |
| Self-authored browser fixture generator | ran successfully; scan.pdf and solid.pdf created in a new temporary directory |
| Real Wasm/build/browser gate | NOT RUN; prerequisite probe exited 2 (blocked) |

Rust counts total 96 passing tests across these filtered/native targets, not a
full-workspace regression. Later re-runs of the same tests do not add to that
count. Large source scans encode/decode via scanlines/native reduced IDCT;
dimensions and output budgets are checked, but no process peak-RSS measurement
was performed. Existing private PDFs were not copied into fixtures.

Logs on this machine: `/tmp/rzpdf-resources-{native,image-unit,images,dct,regions,
resolution-final,tsc,sdk,worker,viewport,text,fixtures,build-tests}.log`.
The final resolution-only repeat also passed all four tests. Logs are temporary;
the commands in [the guide](resources.md#verification-commands) reproduce checks.

The normal all-workspace/all-target Clippy run exited 0 with warnings. Strict
`-D warnings` did **not** pass: its diagnostics include existing-code locations
in shading, JPEG/JPX, font, security and enum lint expectations. The final normal
JSON contains 61 unique primary diagnostic locations, none on new/changed lines
or newly added files. This is a differential location check, not a separately
compiled clean-baseline proof. No global warning suppression or unrelated lint
cleanup was performed; do not advertise a clean strict lint gate.

### Remaining gate / required environment

`scripts/check_webassembly.py --output /tmp/rzpdf-resources-wasm-readiness.json`
reported missing emcc, em++ and an installed Emscripten target. This checkout has
no Emsdk output directory, generated Wasm module or local Web npm dependencies.
The installed Node is v24.8.0, while the locked build expects v22.15.1 and the
Emsdk Node in toolchain.json. An installed nightly alone is insufficient.

At the initial blocked checkpoint no SDK, target, Node version, npm package or
browser was downloaded/installed. The subsequent user authorization permits the
pinned environment setup; real build/browser results must supersede this block.
With an authorized/provided locked environment, build the **new** module using
`scripts/build_webassembly.py`, generate fixtures with resource_scan and run
`web/tests/resources-browser.mjs` through both real transports. Preserve its
report, source/module hashes and any failures before closing WR-07. Old browser
acceptance reports and the new stub script cannot close this gate.

## Authorized environment setup and real browser acceptance

This section supersedes the initial missing-environment block above. The user
explicitly authorized installation and continued verification on 2026-10-25.

- Emsdk installed/activated inside ignored `output/webassembly/toolchains/emsdk`,
  at commit `d7b4130688bfa348392b921ab04015aa15c4f154`, Emscripten 3.1.64,
  SDK Node 24.19.0 and SDK Python 3.13.3. No shell startup file was edited.
- Runtime Node 22.15.1 is project-local, selected only with a scoped PATH. The
  Darwin x64 archive SHA-256 matches the official SHASUMS256 entry:
  `1c722d0dd6d3f60e8b0be014ea01b8a59f5088f4419197a1b37544854d61cc6f`.
- The existing nightly-2026-08-26 matches rustc commit
  `787af2b8c80638c51a4fc8e44f84e6891f243ec7`; rust-src was already installed.
  The target standard library was rebuilt with build-std; a precompiled target
  installation is not needed for this route. Native default toolchain unchanged.
- `npm ci --prefix web --ignore-scripts --no-audit --no-fund` installed only the
  lockfile's TypeScript 5.9.3 and playwright-core 1.46.1. Cargo.lock and the npm
  lockfile are unchanged. Existing Chromium 148.0.7778.96 was reused; no browser
  download or system security/sandbox setting change was made.

Actual setup exposed two build-driver issues, now covered by tests: cold SDK
sanity output preceding the version line, and Cargo warning stderr preceding
machine JSON. Version checks still enforce the pinned version; JSON capture now
parses stdout separately while preserving stderr. No user Cargo config changed.
The build/prerequisite verifier suite now passes 11 tests (the earlier nine
remain in the historical checkpoint above).

The first successful build is `output/webassembly/resource-build-run3.json`:
optimized `wasm-release`, 418.966 seconds, 18 target native archives sampled and
verified as Wasm. The two earlier setup-failure reports are retained as
resource-build.json and resource-build-verified.json, not counted as passes.
Pinned target-feature/linker initialization warnings remain visible.

Real gate: `web/tests/resources-browser.mjs`, Node 22.15.1 and Chromium
148.0.7778.96; `output/webassembly/resource-browser/report.json` records passed,
zero page errors, both real transports, actual artifact/fixture/driver hashes.
It verifies independent 576/1152-DPI solid tile pixels, effective conservative
limits, repeated-tile cache hits, cache release without invalidating caller
pixels, 8.75M-pixel scan preview and text/font queries, structured image-limit
metadata through Worker, lower-DPI recovery and native-handle release.

The module is about 12 MiB on disk (not its runtime footprint), with the unchanged
256 MiB maximum linear memory. No peak-RSS or all-codec claim is made.
Wasm SHA-256: `6d4412d947571965913002b74dfc4d5c29d8f2ead7a49ea246f1ae5555105ee6`.
Module JS SHA-256: `b2e55f2070e754e21a5e860dde488cbe9ecf932db5f29e99df058a5d5b6739f6`.

Final repeat: `output/webassembly/resource-build-final.json` plus the browser
report must agree on those module hashes; Markdown acceptance notes can differ
from the first build snapshot, but runtime-source and SDK fingerprints must
match the final build inventory. No commit, push or release was performed.
