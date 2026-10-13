"""Independently verify the v0.3 embedded static-font subset sample.

Usage: python3 docs/check-v03-font-subset.py OUTPUT.pdf
Requires FontTools 4.39.4, mutool and qpdf. This checks fixed glyphs only;
it is not a general TrueType validator.
"""

import argparse
import hashlib
import subprocess
import tempfile
from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "pdf/tests/fixtures/v03"
CJK = FIXTURES / "NotoSansSC-static-subset.ttf"
LATIN = FIXTURES / "RzpdfTestSans-composite.ttf"
EXPECTED_HASHES = {
    CJK: "cc677006b8f95153fe9bb59107ae7b201e71796181bffbaeb062a858d8a39942",
    LATIN: "b400c05b3164fb6407b258b2438e27670dfb30ddb2cb9047cf7baecb91c3b534",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def checked_font(path: Path) -> TTFont:
    data = path.read_bytes()
    padded = data + bytes(-len(data) % 4)
    checksum = sum(
        int.from_bytes(padded[index : index + 4], "big")
        for index in range(0, len(padded), 4)
    ) & 0xFFFFFFFF
    assert checksum == 0xB1B0AFBA, f"global SFNT checksum: {path}"
    font = TTFont(path, checkChecksums=2)
    for tag in font.reader.keys():
        font.reader[tag]  # Raises on a bad table checksum.
    assert "glyf" in font and "cmap" in font and "hmtx" in font
    return font


def compare_glyph_gid(source: TTFont, edited: TTFont, gid: int, seen: set[int]) -> None:
    if gid in seen:
        return
    seen.add(gid)
    source_name = source.getGlyphOrder()[gid]
    edited_name = edited.getGlyphOrder()[gid]
    source_glyph = source["glyf"][source_name]
    edited_glyph = edited["glyf"][edited_name]
    assert source_glyph.compile(source["glyf"]) == edited_glyph.compile(edited["glyf"]), gid
    assert source["hmtx"][source_name] == edited["hmtx"][edited_name], gid
    assert source_glyph.isComposite() == edited_glyph.isComposite(), gid
    if edited_glyph.isComposite():
        for component in edited_glyph.components:
            compare_glyph_gid(source, edited, edited.getGlyphID(component.glyphName), seen)


def compare_repertoire(source: TTFont, edited: TTFont, text: str) -> set[int]:
    source_map = source.getBestCmap()
    edited_map = edited.getBestCmap()
    checked: set[int] = set()
    for char in text:
        source_gid = source.getGlyphID(source_map[ord(char)])
        edited_gid = edited.getGlyphID(edited_map[ord(char)])
        assert source_gid == edited_gid, (char, source_gid, edited_gid)
        compare_glyph_gid(source, edited, source_gid, checked)
    return checked


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pdf", type=Path)
    args = parser.parse_args()
    pdf = args.pdf.resolve()
    for path, digest in EXPECTED_HASHES.items():
        assert sha256(path) == digest, f"fixture changed: {path}"
    subprocess.run(["qpdf", "--check", str(pdf)], check=True, capture_output=True)
    with tempfile.TemporaryDirectory(prefix="rzpdf-v03-font-") as directory:
        folder = Path(directory)
        subprocess.run(
            ["mutool", "extract", "-a", str(pdf)],
            cwd=folder,
            check=True,
            capture_output=True,
        )
        extracted = sorted(folder.glob("*.ttf"))
        assert len(extracted) == 4, f"expected four embedded fonts, got {len(extracted)}"
        fonts = {path: checked_font(path) for path in extracted}
        originals = {}
        edits = {}
        for path, font in fonts.items():
            digest = sha256(path)
            if digest == EXPECTED_HASHES[CJK]:
                originals["cjk"] = font
            elif digest == EXPECTED_HASHES[LATIN]:
                originals["latin"] = font
            else:
                name = font["name"].getDebugName(6)
                if name == "NEWGLY+NotoSansSC-Thin":
                    edits["cjk"] = font
                elif name == "COMPOS+RzpdfTestSans-Regular":
                    edits["latin"] = font
                else:
                    raise AssertionError(f"unexpected embedded font: {name}")
        assert set(originals) == {"cjk", "latin"}
        assert set(edits) == {"cjk", "latin"}
        cjk_gids = compare_repertoire(originals["cjk"], edits["cjk"], "你新")
        latin_gids = compare_repertoire(originals["latin"], edits["latin"], "Café")
        composite_gid = originals["latin"].getGlyphID(
            originals["latin"].getBestCmap()[ord("é")]
        )
        assert originals["latin"]["glyf"][originals["latin"].getGlyphOrder()[composite_gid]].isComposite()
        assert len(latin_gids) > len(set("Café")), "component closure was not traversed"
        print(f"CJK glyph closure: {sorted(cjk_gids)}")
        print(f"Latin glyph closure: {sorted(latin_gids)}")
        print("all table/global checksums, original GIDs, outlines and hmtx metrics pass")


if __name__ == "__main__":
    main()
