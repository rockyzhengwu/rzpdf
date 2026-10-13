# Bundled font and mapping data provenance

The 14 files under `fonts/*.pfb` are byte-identical to the corresponding files in
the Debian-distributed upstream archive
[`gsfonts_8.11+urwcyr1.0.7~pre44.orig.tar.gz`](https://deb.debian.org/debian/pool/main/g/gsfonts/gsfonts_8.11+urwcyr1.0.7~pre44.orig.tar.gz).

Verified on 2026-10-13 with `cmp` for every PFB. Archive SHA-256:
`9f2a598998bf05e023546ac981aa2a857aa1635d2e0e3020a3c3004ad564dc00`.
No bundled font binary was replaced.

The upstream archive contains GPL version 2 in [COPYING](licenses/gsfonts-COPYING),
and describes its document-embedding exception in [README](licenses/gsfonts-README).
[README.tweaks](licenses/gsfonts-README.tweaks) is also retained. Text is copied
without substantive changes; the missing final newline in upstream README is
normalized in this repository.
Copyright notices remain in each font, including URW++ and Valek Filippov.
These files are not relicensed under the repository's root LICENSE.

This is a source/notice inventory, not a blanket statement about downstream
licensing obligations. Modern URW font releases are not the provenance reference
for these historical binaries.

## Adobe Core 14 AFM metrics

All 14 `afm/*.afm` files match the corresponding Apache PDFBox comparison
files with whitespace/blank lines ignored (`diff -w -B`). No metric differences
were found. Comparison commit: `7e6236667447bf17b9ab3bc5191ed6b04388a8cc`;
[upstream directory](https://github.com/apache/pdfbox/tree/7e6236667447bf17b9ab3bc5191ed6b04388a8cc/pdfbox/src/main/resources/org/apache/pdfbox/resources/afm).
The repository copies have different whitespace/blank-line formatting; this is
not a byte-identical provenance claim. Existing Adobe copyright notices remain.
The accompanying upstream [MustRead.html](licenses/Adobe-Core14-MustRead.html)
is retained without changing its permission paragraph. It is not the PFB license.

## CMap data: release review still open

54 of the 70 `cmaps/*` files retain Adobe copyright and redistribution notices.
Their distinct, verbatim notice blocks are retained in
[Adobe-CMap-NOTICES.txt](licenses/Adobe-CMap-NOTICES.txt), including the
1990-2015 and 1990-2017 copyright variants. Binary distributors must also retain
applicable notices; `build.rs` strips comments from generated CMap byte arrays.

The following 16 files have no embedded permission statement:

`GBK-EUC-H`, `GBK-X`, `GBK2K-H`, `GBKp-EUC-H`, `UniCNS-UCS2-H`,
`UniCNS-UTF16-H`, `UniCNS-X`, `UniGB-UCS2-H`, `UniGB-UTF16-H`, `UniGB-X`,
`UniJIS-UCS2-H`, `UniJIS-UTF16-H`, `UniJIS-X`, `UniKS-UCS2-H`,
`UniKS-UTF16-H`, `UniKS-X`.

On 2026-10-13, all 16 matched the same-named files byte-for-byte in
[MuPDF resources/cmaps](https://github.com/ArtifexSoftware/mupdf/tree/a8afa1d1ad5db3809ad3aaca185dc0f1476e9a27/resources/cmaps),
commit `a8afa1d1ad5db3809ad3aaca185dc0f1476e9a27`. MuPDF's published project
license is [AGPL](https://mupdf.readthedocs.io/en/latest/license.html).
This content match does not prove the historical copying route or establish
a separate permissive license for those files. Do not assign them Adobe BSD
terms merely because other CMaps carry those terms, or the repository's root
GPL terms merely because they are bundled here.

Confirm the applicable terms and distribution obligations, or replace these
files from a verified licensed source, before closing V01-08 and publishing a
release. No mapping bytes or repository root license were changed in this audit.
