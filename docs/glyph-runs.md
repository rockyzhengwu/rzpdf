# Explicit PDF glyph runs

Use `PdfDocument::overlay_page_glyph_run` to append independently positioned
characters with a font already registered on the target page. This is a PDF
character-code API, not a shaping engine or a way to treat GIDs as PDF codes.
The same method is available through the owned `Document` facade.

```rust
use pdf::document::glyph_run::{GlyphRun, PositionedGlyph};

// `font_name` must refer to an existing page font. Here its encoding maps
// PDF code 0x41 to A. Other fonts, especially CID fonts, use different codes.
let glyphs = [PositionedGlyph {
    code: b"A",
    expected_gid: None,
    unicode: Some("A"),
    position: [40.0, 80.0],
}];
let run = GlyphRun {
    font_name: &font_name,
    font_size: 20.0,
    matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    color: [0.0, 0.0, 0.0],
    glyphs: &glyphs,
};
document.overlay_page_glyph_run(0, &run)?;
```

## Codes and assertions

Each `code` must decode to exactly one character under the target font's
existing PDF encoding. Simple fonts accept one byte; Type0 fonts use their
CMap. The bytes are not UTF-8, a CID, or a GID unless that font explicitly
defines an identity mapping. No font resource, CIDToGIDMap or ToUnicode is
rewritten by this operation.

`expected_gid` optionally checks the existing code -> CID -> GID result.
`unicode` optionally checks the existing extraction mapping, including exact
multi-scalar strings. Neither field changes the encoding. Missing Unicode
is distinct from an explicit U+FFFD; an assertion against a missing mapping
fails. Missing font programs, declared substitute fonts, .notdef, out-of-range
GIDs, Type3 writing and unavailable numeric mappings reject before editing.
Bundled Core14 and the embedded combinations in the
[v0.7 matrix](releases/v0.7-font-matrix.md) are supported.

Use `PdfPage::inspect_font` or `inspect_font_with_budget` to inspect the current
page resource. `InspectedFont` dereferences to the existing `Font` API for
code decoding, glyph IDs, Unicode, source and metrics. It owns an inspection
snapshot and its retained quota; inspect again after changing resources.
It is not a writer binding. New runs always resolve the target page afresh.
Calling legacy `Font::clone` through this snapshot creates an ordinary,
separately untracked clone; it does not clone the snapshot's reservation.

## Coordinates and state

`position` is an absolute text-origin offset before the run's affine matrix,
in user units rather than em or font-program units. The matrix maps these
offsets into unrotated default page user space. Page rotation, CropBox and
UserUnit still apply on display. Each glyph gets its own Tm/Tj, so one glyph's
advance never positions its neighbour. Font size is finite and positive;
positions/matrices must be finite, and RGB components must be within 0..1.

The writer explicitly selects Tr=0, Tc=Tw=Ts=TL=0 and Tz=100 in isolated
graphics/text state. It does not inherit the old content's CTM or clip.
Vertical fonts retain their W2 glyph-origin offsets. This does not apply
vertical OpenType substitutions or infer line layout.

Extraction's `ExtractedCharacter::position` remains the advancing **text
origin**. `vertical_origin` holds the W2 vector in thousandths of text space.
`glyph_origin_display()` returns the horizontal outline origin after W2 and
page transforms; `em_to_display_matrix()`/`display_to_em_point()` and
`TextObject::character_outline_display()` now use that origin for vertical
fonts. Horizontal behavior is unchanged. Adding the public extraction field
requires downstream struct-literal constructors to provide it.

## Failure isolation and budgets

All input and font checks precede publication. The overlay uses page-scoped
content replacement, so shared page streams and Form-local resources are not
mutated. An outer edit transaction can roll it back. Unsigned/edit-permission
checks run before preparation; signed input and unauthorized R6 users reject.
Full save and existing unsigned unencrypted incremental save are supported;
encryption retains the existing full-save rules, not encrypted incremental save.

`overlay_page_glyph_run_with_budget` shares one pool across resource/font
resolution, the native face's Rust input copy, selection serialization and
output capacity. Output sizing is a stack-only pass followed by one reserved
allocation; no per-glyph retained vectors or Unicode copies are needed.
Glyph count, operation count, token, Unicode assertion and stream lengths are
bounded by document limits. The input slices belong to the caller. Native
allocations, legacy overlay validation copies, page traversal and persistent
document storage retain their existing accounting exclusions; this is not a
process RSS or CPU ceiling.

## Verification

```sh
cargo test -p pdf --locked --test v07_font_mapping --test v07_glyph_run -j2
cargo build -p pdf --example v07_font_roundtrip -p render --example render_document --locked -j2
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v07-fonts.py
```

The fixed driver writes a new output file, never overwriting one. The checker
uses independent PDF operators for expected positions and pixels, FontTools
for font paths/widths and MuPDF for GIDs, Unicode and W2 origins. Evidence is
created in a fresh `tmp/pdfs/v07-fonts-*` directory; inspect the page PNGs as
well as the assertions. No Poppler or fuzz is used.
