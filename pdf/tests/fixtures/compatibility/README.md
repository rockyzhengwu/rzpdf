# Compatibility fixtures

Synthetic, repository-generated fixtures; no private document content.
The three scans use an 8.75 megapixel source on a 612 × 792 point page.

- `scan-flate.pdf`: constant RGB 121, cumulative source samples 26,250,000 bytes.
- `scan-jbig2.pdf`: packed MMR JBIG2 stencil, opaque red on the left, one-pixel strokes every four source columns on the right. Area previews must retain roughly 25% coverage over the background, rather than losing these strokes.
- `scan-ccitt.pdf`: Group 4 stencil, left half opaque red and right half transparent over RGB (179, 204, 230).

Regenerate with `cargo run -p pdf --example compatibility_fixtures -- pdf/tests/fixtures/compatibility`.
The manifest pins input bytes. These cases are feature tests, excluded from real-document coverage rates.

`hint-reference.pdf` and `hint-failed.pdf` embed our own geometric TrueType font.
The latter contains a failing hint CALL; its valid rectangle outlines must remain
visible. No proprietary font data is redistributed. Regenerate these two PDFs
with `python3 scripts/compatibility_hint_fixture.py pdf/tests/fixtures/compatibility`
using fontTools 4.39.4 (an optional regeneration dependency).
