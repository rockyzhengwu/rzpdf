# Pattern paints

Pattern painting is experimental while v0.8 is in progress. Tiling Pattern and Shading Pattern definitions can be registered with `ResourceKind::Pattern`, imported with `import_resource_from`, selected by `scn`/`SCN`, and retained in interpreted display lists. The original PDF resource graph, not the interpreted snapshot, is serialized on save.

## Selection and coordinates

Use `/Pattern cs /P1 scn` for a coloured pattern, or register `[/Pattern /DeviceRGB]` and use `/CS1 cs r g b /P1 scn` for an uncoloured tiling pattern. Stroking uses `CS` and `SCN`. A missing selection, wrong component count, mismatched PaintType, missing local resource, or malformed dictionary is an error.

The pattern matrix maps to the default coordinate system of its defining content stream. A later `cm` changes the shape being painted, not the selected pattern's lattice. A Form establishes its own default coordinate system at invocation. Explicit pattern Resources are local; missing names do not fall back to the page.

The interpreter publishes an immutable `PatternPaint` only after successful interpretation. It contains page-display-coordinate graphics under an `Arc`; cloning the parent graphics state shares this snapshot. These graphics do not expose page-edit provenance. The original path or text object retains its own provenance.

## Drawing and limits

The current implementation expands cells intersecting the page's enclosing rectangle, clips each to BBox, and charges repeated operations, decoded work, object count, retained segments, and logical payload. Positive and negative steps, gaps, overlapping BBoxes, and matrix transformations are represented without replacing the pattern with a solid colour. Cell count and recursion are bounded, including empty cells.

Skia renders the snapshot to a page-sized source and samples it in device coordinates for a path, text, or stencil paint. Normal-blend definitions use the standard's isolated optimization; backdrop-dependent definitions use a non-isolated group with the actual current backdrop, removing it before object compositing. Live parent, sibling shaders and nested group/correction surfaces are charged before allocation. Fractional DPI tiling uses 3× supersampling and explicit premultiplied 3×3 area averaging, not bilinear minification; this avoids white seams in the frozen overlap cases. This is a conservative logical surface allowance, not a bound on total RSS or every backend/font/legacy temporary. Page-sized snapshots and repeated compilation are not a performance guarantee; cell caching and narrower surfaces remain optimization work.

Uncoloured cells inherit their base-space tint for filling and stroking. Colour-setting operations, shading calls, and non-stencil images in such cells are rejected, including through Form calls. Path clipping remains independent of colourant paint suppression.

## Shading Pattern

PatternType 2 accepts Shading, Matrix and the currently supported ExtGState entries. Its Background applies where the shading itself does not cover, within a declared shading BBox. Direct `sh` continues to ignore Background. The two routes are intentionally distinct, as specified by [PDF Reference 1.5 table 4.24](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf).

Installed MuPDF 1.27 paints Background even for direct `sh` in the fixed test. The independent gate retains this mismatch and also renders a standard-equivalent reference with only that ignored entry removed. Its low-resolution axial shading tessellation moves an endpoint by one pixel; the gate retains native output and samples 576 dpi at the 72 dpi pixel centers for the numerical comparison. It does not raise the colour threshold to accept those native differences.

## Verification

Build `v08_patterns` and `render_document`, then run:

```sh
cargo test -p pdf --test v08_pattern --locked -j2
cargo test -p render pattern_tests --locked -j2
cargo build -p pdf --example v08_patterns -p render --example render_document --locked -j2
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/check-v08-patterns.py
```

The Python environment needs Pillow and pikepdf; the gate also needs qpdf and mutool. Every run creates a new evidence directory under `tmp/pdfs`. Combined backdrop/mask/blend gates and final status are listed in [the v0.8 support matrix](releases/v0.8-support.md).

## API changes

`ResourceKind::Pattern` and the payload of `Pattern::Shading` change exhaustive downstream matches. `TilingPattern` exposes validated geometry/resources and no longer has a misleading zero-step Default. `PatternColorSpace::try_new` supports direct definitions; use `try_new_with_context` for referenced definitions or the ordinary `ColorSpace::try_new` entry. No v1.0 API compatibility guarantee is implied.

`PatternColorSpace` is no longer an empty marker struct: use its constructors
or `Default` rather than an empty literal. `Default` has no selected Pattern;
attempting to paint it is an explicit error, not a black colour substitute.
