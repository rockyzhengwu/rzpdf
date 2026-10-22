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

The legacy CCITT filter exposes expanded 0/255 samples. The common image
adapter converts these to row-padded one-bit samples before applying Decode
and stencil semantics. Inline Group 4 with an ASCIIHex wrapper is covered,
including widths not divisible by eight. Direct Group 4 inline framing also uses decoded row/EOB boundaries through a counting sink, with bounded geometric lookahead and scratch reservations; it never searches encoded bytes for EI. Group 3 direct inline framing remains unsupported.

## Image soft masks and Matte

Image `SMask` accepts independently sized, non-stencil `DeviceGray` images with
1/2/4/8/16-bit samples and a valid `Decode` array. Packed mask samples retain
their precision until conversion; the final RGBA buffer has 8-bit alpha.
Nested `Mask`/`SMask` remain unsupported. Masks retain their own packed dimensions and pixel limits. Extraction samples opacity at base-image pixel centers; rendering uses the larger width/height of image and masks, samples source colors and alpha independently, and unblends Matte before color conversion. The combined render grid has a checked RGBA byte limit; it does not allocate an intermediate base RGBA image.

An optional `Matte` array describes preblended colour. The image adapter
inverts `c' = m + alpha * (c - m)` after sample decoding, **in the source colour
space before RGB/ICC/output-profile conversion**. Indexed images expand their
palette first: Matte describes the base-space colour, not the palette index.
Recovered components are clipped to the source-space ranges; exactly zero
alpha uses transparent black without division. Nonzero alpha that rounds to
zero in 8-bit output is still used at full mask precision for unblending.
Matte values must be finite, in range, and match the source/base component
count; indirect arrays/components are resolved, and retained Matte storage is
included in image payload accounting. Original stream objects remain unchanged
for saving. `PdfImage::data()` is not an unmatted RGB buffer; use RGBA conversion
for display-ready pixels.

The rule is defined in ISO 32000-1, 11.6.5.3; see also Adobe's
[soft-mask image description](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf).
Deterministic coverage lives in `pdf_image_matte_tests.rs`; it includes RGB,
Gray, CMYK, CalGray/CalRGB, Lab, ICCBased, tint spaces, Indexed palettes,
Decode, packed row padding, precision and malformed parameters. This does not
expand the existing codec or general transparency-group colour-space envelope.

## Limits and dependencies

Explicit budget entry points share `OperationBudget` with Globals and retained
output. Header preflight rejects excessive input, dimensions and structural work
before backend pixel allocation. Conservative scratch reservations do not
intercept every backend Vec allocation or enforce an RSS/CPU hard cap. Use
process isolation and independent resource/time limits for untrusted-file
services; these APIs are not a sandbox. Legacy image interpretation still has
the untracked/native allocation exclusions of the existing image pipeline.

The default build pins hayro-jpeg2000 0.4.1 and hayro-jbig2 0.3.1 with `std`, and hayro-ccitt
0.4.0 without optional features; optional image integration and SIMD remain disabled. Their upstream
license is MIT OR Apache-2.0; [the MIT notice](licenses/Hayro-MIT.txt) is retained
for distributors using that license option. Project AGPL-3.0-only and bundled
font/CMap licenses remain unchanged. Rust 1.92 or newer is required by these
codec dependencies. There is no runtime "backend disabled" branch in the
default build; a missing dependency is a build error, not blank-image success.

## Corpus compatibility follow-up

A terminal JPX stage may be preceded by bounded byte filters (ASCIIHex, ASCII85, Flate, LZW, RunLength and authenticated Crypt). Header inspection runs after those wrappers, with the ordinary filter/DecodeParms validation retained. A bounded palette extension supports one unsigned 8-bit codestream index component, up to 256 unsigned 8-bit Gray/RGB entries, and explicit sequential component mapping. Sequential channel definitions support straight Gray/RGB/CMYK opacity. `SMaskInData 1` extracts the trailing opacity channel; absent/zero ignores it. CMYK samples use the existing source-color conversion. Premultiplied opacity (`SMaskInData 2`), conflicting explicit masks, ICC specifications and other unsupported JPX coding envelopes remain rejected. Channel-separation buffers are checked against the decoded-byte limit. Explicit stencil masks retain their native sample resolution for rendering; extraction retains the original base-image dimensions. Combined mask/base RGBA allocations remain bounded.

The renderer honors the image Interpolate flag (false by default): nearest sampling when false, linear sampling and bounded minification mipmaps when true. Mask extraction keeps base-image dimensions; rendering may use a finer mask grid. This does not promise identical minification to other rasterizers.


The single-tile JPX envelope permits a nominal tile larger than the image, with
zero image/tile origins. The backend clips tile work to the image bounds; sample
and scratch reservations continue to use the image dimensions. Component coding
(COC) is accepted only when its component index is valid and all coding settings
exactly equal the main COD settings. Different settings, nonzero origins and
multiple tiles remain unsupported. This extension does not raise resource limits.

PDF Indexed JPX images keep the original indices and use the PDF palette in
preference to the JP2 palette. Unsigned full-resolution 1/2/4/8-bit source index
planes are accepted, with indices retained as bytes without gray scaling. JPX
Decode remains ignored. Other explicit PDF color spaces require a matching
source color-channel count; incompatible counts remain errors.

Compatible mode may remove a single CR, LF or CRLF surplus after the exact image
or soft-mask sample count, recording an image trailing-byte diagnostic. It never
pads missing samples or drops non-line-ending bytes; Strict retains exact length
validation. The bounded recovery also applies after byte-wrapper filter chains.

JPEG header dimensions are checked against provided image dimensions before
reserving or decoding the pixel buffer, including indirectly referenced fields.
A large or contradictory JPEG does not allocate pixels before that check.

A malformed document can still be rejected after a codec limitation is fixed.
Dictionary sample geometry, color counts, Matte dimensions, and actual decoded
row lengths remain independently validated; successful header decoding never
permits truncated or contradictory pixel data.
