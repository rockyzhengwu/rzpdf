# Bounded image previews (RZPDF-003)

Follow-up: [RZPDF-004 masked previews](masked-image-preview.md) extends the original
unmasked boundary documented here.

Scope: render-only target decoding, with the existing default browser budgets.
Source PDF streams, full-fidelity extraction and save/edit behavior stay separate.

Implementation and acceptance checklist:

- [x] Common target sizing from the image-to-display transform and DPI.
- [x] Independent source-pixel, inflated-byte, target-pixel and RGBA checks.
- [x] Flate 8-bit DeviceRGB/DeviceGray scanline decode and integer area sampling.
- [x] Predictor 1, TIFF 2 and PNG 10–15 with matching image row geometry.
- [x] Decode mapping before sampling; PNG history retains reconstructed samples.
- [x] Verify exact decoded size and zlib StreamEnd/checksum before publication.
- [x] Decode result records source/output sizes and retains sample budget ownership.
- [x] Keep reduced-image cache entries dependent on the draw transform.
- [x] Add image error context and expose it through SDK/Worker.
- [x] Native image, allocation, malformed data, save and renderer regressions.
- [x] Fixed fixture native/synchronous SDK/Worker acceptance at default limits.
- [x] Cancellation/recovery, unsupported combinations and existing JPEG gates.

The first slice supports a single Flate filter, 8-bit DeviceRGB/DeviceGray,
Predictor 1 or row-matched Predictor 2/10–15, and finite Decode endpoints. Masks,
complex color spaces/device-default overrides, other component depths/filter chains and unsupported
predictor layouts keep the normal bounded path. Progressive JPEG target decoding
is deliberately unavailable. Ordinary JPEG retains its existing IDCT choices.

Target decoders retain a 16,000,000 source-pixel work guard. Flate's total inflated
bytes (including PNG row tags) must still fit maxDecodedBytes; streaming does not
raise this ceiling. The operation pool covers raw read/decryption owners when
needed, target samples, current/previous rows and both sampling accumulators.
Zlib-private fixed state and fixed stack lookup tables are excluded from that
logical accounting. Raster/Skia storage has its existing independent limits.

Native image errors wrap their original cause; use PdfError::root_cause() for
classification and image_decode_details() for context. Browser errors preserve
code/reason/stage/limit/actual and add resource.image with encoding, source width
and height, targetWidth/targetHeight, targetDecodeSupported, unsupportedReason
and jpegProcess. Support describes the decoder route, not a promise that any
requested target fits the active budgets.

## Acceptance and reproduction

The committed `pdf/tests/fixtures/image-preview/manifest.json` pins the original
three editor-request hashes. The 22,601-byte Flate fixture has SHA-256
`48b6c5d3ee04b49abdf5595e220065dfc4db6479eea1b318e8e9743bc0fbcc9b`.
It contains 5,106,140 source pixels and 15,318,420 RGB sample bytes. Decoder-pool
peak for its 595 x 842 preview is below 2 MiB in the native allocation test;
the 15.3 MB source RGB and 20.4 MB full-source RGBA buffers are never constructed
by the streaming preview. Raster output and decoder-private state are separate.

The original fixtures omit the binary header comment required by this engine's
Strict policy. Open them in Compatible mode to preserve the request hashes.
Ordinary saves are checked with Strict reopening. A fourth fixture flips the
final zlib checksum byte and must fail decoding even though qpdf accepts its PDF
structure. Python zlib independently rejects that checksum and accepts the valid
Flate payload; all four generated files reproduce their pinned hashes.

```sh
cargo test -p pdf --lib filter::flate_image
cargo test -p pdf --test image_preview --test render_image_resolution
# After the pinned optimized Wasm build and npm --prefix web run build:
node web/tests/image-preview-browser.mjs
```

Native full regression: 1,864 tests passed with one existing ignored test.
Additional page-level checks include the device-default override exclusion and
shared preview ownership. `cargo clippy -p pdf -p render -p web-wasm --lib --
-D warnings`, SDK resource/protocol tests, Worker lifecycle tests, TypeScript
compilation and build-driver checks passed.

Chromium synchronous/Worker acceptance records three rounds at 72/36/18 DPI,
independent color probes across every white stripe, per-frame hashes, budgeted
fallback, viewer-rotation context and actual page rotation/restoration, cache
limits, unmodified render revision, strict saved reopen, source/output budget
refusals, progressive JPEG negatives, late-checksum failure atomicity, and active
Worker cancellation followed by session recovery. The conservative profile
continues to reject this RGB source above its 8 MiB inflated-byte limit; lowering
DPI cannot bypass that independent source-work budget.

Evidence is retained under `output/image-preview-20261029/`. Existing raw-source
residency, sequential JPEG/high-DPI resources, Unicode FreeText and linearized
save browser regressions also passed. These are engine and SDK results; editor
application acceptance remains downstream after pinning and rebuilding.

The first high-source-pixel slice covers Image XObjects. Inline-image parsing
retains its own existing source-pixel admission limit. This change does not lift
that parser limit; eligible inline images already admitted by it may reuse the
render-only target decoder.
