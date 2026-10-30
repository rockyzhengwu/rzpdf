# Bounded masked image previews (RZPDF-004)

This extends [RZPDF-003](image-preview.md) with render-only joint color/alpha
sampling. It does not raise the browser's 4,000,000 image-pixel, 16 MiB decoded,
64 MiB operation or 256 MiB linear-memory limits.

## Implementation boundary

Supported: single FlateDecode/Fl filter, 8-bit DeviceRGB/DeviceGray base image,
8-bit DeviceGray image SMask, Predictor 1 or row-matched TIFF 2/PNG 10–15,
finite Decode endpoints, and optional Matte in the base color space. Base
Interpolate remains attached to the final preview and drawing path. Source-mask
Interpolate=true is explicitly outside this first slice; existing nearest-cell
mask reconstruction is used otherwise. Image XObjects are supported; inline
source-pixel parsing limits, Type3 and tiling-pattern exclusions stay in force.

Explicit hard masks, color keys, standalone stencils, embedded alpha, JPEG/JPX
masked target decode, non-device colors/default overrides, other component
depths and filter chains retain bounded full-fidelity decoding or a typed error.
A non-null SMask takes precedence over Mask, as in the full-fidelity path.

The target uses the image transform and DPI, bounded by the larger dimensions
of the base and mask. A one-pixel color image can therefore retain detail from a
high-resolution mask. Independent forward-only Flate readers retain at most the
current and previous predictor rows of each source. The sampler merges base,
mask and target cell boundaries in image space, accumulating opacity-weighted
color and opacity. Matte is removed in source color space before accumulation;
zero alpha never requires division. Only target-size straight color samples and
alpha are published, with shared budget leases. Neither source RGB/RGBA nor a
full-source alpha array is generated. Original streams, dictionaries, extraction
and save/edit semantics remain unchanged.

Each source independently has a 16,000,000 pixel work guard; its total inflated
bytes including predictor tags must fit maxDecodedBytes. Target pixels and RGBA
have their existing independent limits. Simultaneous raw/decryption owners,
source rows, target samples/alpha and floating-point accumulators share one
operation pool. Fixed zlib-private state, small stack values, output raster,
Skia and JS storage are outside this decoder accounting. The two retained
accumulators occupy 16 bytes per target component/alpha slot. Before allocation,
the decoder checks simultaneous storage and reports image_operation_bytes.

Very different aspect ratios can make joint traversal much more expensive than
either source alone. A conservative upper bound
`(baseWidth + maskWidth + targetWidth) * (baseHeight + maskHeight)` is limited to
256,000,000, reported as image_sampling_work. This protects CPU work independently
of allocation budgets. Lowering DPI does not remove source decoding limits.

Both readers must validate their final decoded lengths and zlib checksums before
a result escapes. Errors drop all preview leases without publishing a prepared
page. Cache entries retain transform dependence even when the target happens to
match the base dimensions but the mask was reduced. Document revision changes,
including mask edits, invalidate existing preparation caches; different resource
names do not share combined previews solely because they share a mask.

## Diagnostics and capabilities

Dictionary inspection adds resource.image.maskKind (`soft`, `explicit`,
`color-key`, `stencil`, `embedded-alpha`), maskWidth, maskHeight, maskEncoding,
maskBitsPerComponent and mattePresent, without decoding payloads. Unknown or
inapplicable fields are null. The effective mask is reported; ignored Mask
entries behind SMask are not traversed. Unsupported reasons distinguish
mask_encoding, mask_component_depth, mask_color_space, mask_predictor_layout,
mask_interpolate, mask_nested, mask_stencil, mask_explicit, mask_color_key,
mask_embedded_alpha, mask_external_stream and mask_invalid. Cyclic/nested masks
never enter recursive preview decoding.

Capabilities advertise maskedReduction=true, maskedReductionKinds=["soft"],
softMaskEncoding=["flate"], softMaskBitsPerComponent=[8],
softMaskColorSpace=["DeviceGray"], softMaskMatte=true,
softMaskInterpolate=false and maxMaskedSamplingWork=256000000. Consumers must
inspect the supported combination rather than treating maskedReduction as
universal codec/mask support. SDK and Worker carry identical error context.

## Verification and reproduction

Project-generated fixtures under pdf/tests/fixtures/masked-preview contain a
2665 x 1916 RGB base (>4 million pixels), full-sized alpha and transparent,
half-transparent, opaque and gradient regions. They include white/dark/checker
backgrounds, white Matte, a different-sized inverted-Decode mask, and a mask with
a corrupt final checksum. The 768 x 552 point page and checker grid align to device pixels at all three
DPIs; references use exact DPI matrices rather than stretching to rounded canvas
sizes, so background geometry does not dominate transparency comparisons.
The generator uses Python's standard library and no
external assets. The manifest pins every file hash.

Native tests use independent two-dimensional rectangle intersections to check
joint sampling, different grids (including finer masks), Decode and Matte.
They check hidden-color contamination, fully transparent/opaque cases, both
checksums, leases/peak storage, independent budgets/work guards, source
preservation/save reopening, multiple transforms, cache reuse and cyclic-mask
recovery. The 5.1-million-pixel solid pair's decoder-pool peak at 767 x 551 is
below 2 MiB; this is not process RSS or total renderer memory.

```sh
python3 scripts/masked-preview-fixtures.py pdf/tests/fixtures/masked-preview
cargo test -p pdf --lib filter::
cargo test -p pdf --test image_preview --test render_image_resolution
cargo run -p render --example masked_preview -- \
  pdf/tests/fixtures/masked-preview/soft-matte.pdf output/masked-preview-native
# Use the existing benchmark venv, with PyMuPDF and pypdfium2:
output/benchmark-venv/bin/python scripts/masked-preview-references.py \
  pdf/tests/fixtures/masked-preview output/masked-preview-20261030/references
# Build optimized WASM and the SDK first, as in docs/webassembly/build.md:
node web/tests/masked-preview-browser.mjs
```

The browser gate compares complete rendered frames against separately generated
MuPDF/PDFium images at 72/36/18 DPI through synchronous SDK and Worker. It records
mean absolute RGB error and the fraction of pixels with channel error >12;
limits are MAE<2 and fraction<2.5%, allowing different edge filters while still
rejecting missing masks, Matte halos, incorrect opacity or backgrounds. Also
covered: identical transport pixels, cache clearing, default fallback, Strict
save/reopen, late-mask failure atomicity, source/output budget errors, context
fields, active Worker cancellation/session recovery and released handles.

Evidence is written under output/masked-preview-20261030. The originating user
file has not yet been identified locally. Synthetic acceptance does not close
that individual file: confirm its mask type and rerender it, then let pdf-editor
pin the implementation commit, rebuild SDK/Worker artifacts and independently
accept RZPDF-004. Unsupported mask combinations remain explicit follow-up work.


Native acceptance: 1877 tests passed across pdf/render/web-wasm, with
one existing ignored test. After final fixture alignment, all 14 image-preview
integration tests were rerun; the six existing render-image-resolution tests also
passed. Clippy with -D warnings, TypeScript compilation, SDK resource/protocol and
Worker lifecycle tests passed. Native rendering passed 30 full-frame reference
comparisons; synchronous SDK and Worker each passed the same 30 comparisons.
The highest observed mean RGB error was approximately 1.17/255. The final
optimized build receipt, matching source hashes, browser reports and supporting
logs are collected in output/masked-preview-20261030/acceptance.json.
