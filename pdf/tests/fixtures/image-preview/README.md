# RZPDF-003 preview fixtures

Project-generated two-page PDFs, with no external image or font assets. The
second page contains the same 1916 x 2665 RGB pattern as the editor request.
`manifest.json` pins the exact request input hashes.

Generate with Python and Pillow:

```sh
python3 scripts/image-preview-fixtures.py /tmp/image-preview
```

Compare hashes before replacing committed fixtures: JPEG encoder versions can
change encoded bytes. These inputs intentionally retain the request's header
without a binary comment line. Tests open them in Compatible mode, then verify
that ordinary saved output reopens in Strict mode.

`preview-flate-corrupt.pdf` preserves the Flate file's object offsets and lengths,
but flips the final Adler checksum byte. It is an intentional negative fixture;
strict zlib/image decoding must reject it. qpdf may accept its PDF structure
without rejecting the Adler mismatch.
