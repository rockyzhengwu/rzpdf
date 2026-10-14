# Inline images and image codecs

rzpdf interprets typed inline images and a bounded JPEG 2000/JBIG2 baseline in
process. See the [v0.6 input matrix](releases/v0.6-matrix.md) for the exact codec,
colour, bit-depth and structural envelope. Unsupported combinations return
errors; raw object preservation does not imply a rendering promise.

## Inline source and editing

BI/ID/EI is one `Operator`, with no ordinary operands. `inline_image()` exposes
an `InlineImage` carrying the original dictionary, encoded bytes and decoded
content-stream dictionary/data byte ranges. No fictional XObject resource name is allocated.
Unchanged `ContentProgram` bytes remain untouched on save.

```rust,ignore
let mut page = doc.get_page(0)?;
let program = page.content_program()?;
let report = page.extract_images_paint_order_report()?;
assert!(report.matches_program(&program));
let source = report.images[0].source().ok_or("not a page-root draw")?;
let inline = page.extract_inline_image_source(&program, source.operation_index)?;
// inline.encoded_data() is not the decoded RGBA buffer.
doc.remove_page_image_draw(0, &program, source.operation_index)?;
```

`InlineImage::new(dictionary, encoded, limits)` validates framing by parsing a
single content item. `to_content_bytes` serializes explicit inline data;
`ContentBuilder::draw_inline_image` adds an isolated q/cm/BI/Q draw.
`replace_page_inline_image_draw` keeps inline form, while
`replace_page_image_draw` can replace an inline draw with a registered Image
XObject. Existing page-relative/display-relative transforms and nested
image removal/transform/XObject replacement preserve invocation isolation.
Nested raw sources are accessible through `nested_content_program` and its
leaf operator. Source snapshots are document/page/revision qualified; refresh
them after every successful edit. Signed-source and permission guards still
apply. Deletion is logical, not secure erasure.

## Encoded objects and decoders

Normal Image XObject extraction preserves Filter/DecodeParms and encoded bytes.
JPEG 2000 header inference includes width, height, component count, original bit
depth and baseline colour space. For non-stencil JPX images the PDF Decode
array is ignored as specified; inconsistent dictionary dimensions are errors.
JBIG2 decoding accepts bounded region/pattern streams, combines meaningful
Globals with page segments, and returns PDF-polarity MSB-packed samples.
Common symbol-coded JBIG2 is explicitly outside this matrix.

JPX and JBIG2 do not have production encoders in this release. Save/copy/import
retain the source encoding; edits can use normal RGB/RGBA/JPEG output. Shared
Globals are ordinary object references and are remapped during graph import.
No runtime system-tool subprocess is used for production decoding.

## Limits and dependencies

Explicit budget entry points share `OperationBudget` with Globals and retained
output. Header preflight rejects excessive input, dimensions and structural work
before backend pixel allocation. Conservative scratch reservations do not
intercept every backend Vec allocation or enforce an RSS/CPU hard cap. Use
process isolation and independent resource/time limits for untrusted-file
services; these APIs are not a sandbox. Legacy image interpretation still has
the untracked/native allocation exclusions of the existing image pipeline.

The default build pins hayro-jpeg2000 0.4.1, hayro-jbig2 0.3.1 and hayro-ccitt
0.4.0, using `std` without optional image integration or SIMD. Their upstream
license is MIT OR Apache-2.0; [the MIT notice](licenses/Hayro-MIT.txt) is retained
for distributors using that license option. Project AGPL-3.0-only and bundled
font/CMap licenses remain unchanged. Rust 1.92 or newer is required by these
codec dependencies. There is no runtime "backend disabled" branch in the
default build; a missing dependency is a build error, not blank-image success.
