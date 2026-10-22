# v0.6 codec fixtures

The original pixels and PDFs in this directory were generated specifically for rzpdf on
2026-10-14. The channel fixtures described below were added on 2026-10-22. No upstream test images, standards' test bitstreams, scanned text or
third-party PDFs were copied. These project-authored fixtures and generator use
the repository's AGPL-3.0-only license. Producer tools retain their own licenses;
their source code/binaries are not redistributed here.

| Files | Producer and content |
|---|---|
| `rgb-lossless.jp2`, `gray-lossless.jp2`, `rgb-lossy.jp2`, `rgb-raw.j2k` | Pillow 12.3.0 with OpenJPEG; 16x16 project-authored gradients, default lossless or explicitly irreversible rate 5 |
| `openjpeg-gray.j2k` | Separate OpenJPEG `opj_compress` CLI from the same project-authored PGM, three resolutions |
| `generic-mmr.jb2` | Project-authored embedded segment headers, Pillow/libtiff Group 4 encoder from a 32x32 checkerboard |
| `patterns-global.jb2`, `halftone-0.jb2`, `halftone-1.jb2` | Project-authored PatternDictionary/Halftone headers; nonempty shared Globals contains two 8x8 patterns, selected by different 4x4 grids on two pages |
| `jbig2enc-generic.jb2` | Independent arithmetic GenericRegion producer [agl/jbig2enc](https://github.com/agl/jbig2enc/tree/d0dfca46216c98f11312a9c9f15615ed490cd7b3), Apache-2.0; `jbig2 -p our-checker.pbm`, default generic mode, no symbol substitution |
| `external-codecs.pdf` | pikepdf 9.11.0 / libqpdf 12.2.0, nine pages referencing the above encoded streams; pages 7/8 share exactly one Globals object |
| `*.samples` | Unencoded project-authored pixels for exact lossless/polarity checks |

`manifest.json` records SHA-256 of all binary fixtures. The PDF's first two JPX
images deliberately omit ColorSpace/BitsPerComponent, testing header inference.
The other JPX images provide matching dictionary values. No JPX/JBIG2 encoder
is required by production rzpdf.

Reproduction:

```sh
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 \
  pdf/tests/fixtures/v06/generate.py \
  --jbig2 tmp/pdfs/v06-jbig2enc/build/jbig2
```

The optional jbig2enc tool must be built separately; omit that argument only for
an experimental eight-page corpus, not the registered nine-page acceptance
fixture. The generator refuses to replace changed outputs. Maintainers updating
the generator can explicitly use `--refresh-generated`; existing binary files
must still match the previous manifest before replacement. Verify new hashes
and producer provenance after a deliberate update. MMR extraction explicitly
normalizes TIFF's BlackIsZero sample convention to the Group 4/JBIG2 convention.

The additional 4x4 Gray+alpha, RGB+alpha and CMYK fixtures use project-authored
coordinate gradients, produced with Pillow 11.3.0 / OpenJPEG 2.5.3. Run
`python3 pdf/tests/fixtures/v06/generate_channels.py` with those producer versions
to verify byte-for-byte reproduction. The generator preserves the existing
manifest and refuses changed encoded bytes. The original generator retains and
verifies these separately produced entries.

A further 4x4 two-bit index plane uses the project-authored C generator and
OpenJPEG 2.4.0 (`libopenjp2`). Each row contains 0,1,2,3; this verifies that
PDF Indexed samples retain their numeric index rather than scaling to gray.
The generator's header records its compilation and invocation commands.
