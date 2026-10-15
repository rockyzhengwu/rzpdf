# v0.7 font mapping fixtures

`fonts.pdf` is a project-authored PDF 1.7 generated independently with pikepdf
9.11.0 and FontTools 4.39.4. `manifest.json` freezes all file SHA-256 values,
per-page PDF codes, Unicode strings, expected geometric-font GIDs and W/W2
metrics. Reproduce from the repository root:

```sh
PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/build-v07-font-fixtures.py
```

The Python dependencies are verification tools, not Rust runtime dependencies.
Generation is deterministic (fixed font timestamps and PDF IDs). Regeneration
replaces only the four generated fixtures and their manifest in this directory.

## Source and license

`geometry.ttf`, `geometry.otf` and `geometry.cff` have three glyphs: an empty
`.notdef`, a rectangle A and a triangle B. The project-authored outlines and
font metadata use this repository's root license. They are test assets, not
general text fonts or copies of system fonts. Raw CFF and OpenType/CFF have
the same Type2 charstrings; the TrueType version has equivalent glyf paths.

Page 7 embeds the existing Courier-compatible `font-data/fonts/n022003l.pfb`.
The PFB segment headers are removed for PDF FontFile embedding; the segments'
program bytes and original notices are retained. It keeps its own upstream
license and document-embedding exception, not the root license. See
[font-data provenance](../../../../font-data/NOTICE.md) and its linked
COPYING/README notices. The manifest also records the original PFB hash.
Page 8 uses bundled Courier with Differences, without embedding another copy.
No new third-party font was downloaded or added.

## Fixed combinations

Eight pages cover raw name-keyed CFF Type1C, OpenType/CFF, simple TrueType,
non-Identity CIDFontType2 horizontal and vertical, 1/2/3/4-byte code strings,
embedded Type1, and Differences with Unicode deliberately independent from
glyph names. Unicode includes multi-scalar `fi`, a surrogate pair for U+1F600,
CJK and two codes with distinct CIDs sharing one GID. One page rotates 90 degrees.

`v07_font_mapping` checks read/save semantics; `v07_font_roundtrip` appends
explicit glyph runs. `docs/check-v07-fonts.py` checks the same paths with
FontTools, MuPDF/PyMuPDF, qpdf and independently authored PDF text operators.
CID-keyed CFF charset mapping, CFF/Type1 subsetting, shaping and arbitrary
font-format combinations are not positive cases in this fixture.
