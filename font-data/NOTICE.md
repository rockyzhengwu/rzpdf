# Built-in Type1 font provenance

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
