"""Check the fixed v0.3 H/V font matrix with independent PDF/font parsers.

Usage: python3.11 docs/check-v03-font-matrix.py output/pdf/v03-font-matrix.pdf
Requires PyMuPDF and FontTools. This is a fixed-fixture acceptance check, not
a general PDF font validator.
"""

import argparse
import re
from pathlib import Path
from math import isclose

import fitz
from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf"
BFCHAR = re.compile(rb"<([0-9A-F]{4})> <([0-9A-F]{4})>")


def reference(value: tuple[str, str]) -> int:
    assert value[0] == "xref", value
    return int(value[1].split()[0])


def mapped_metrics(pdf: fitz.Document, source: TTFont, font_xref: int, vertical: bool) -> int:
    descendant = reference(("xref", re.search(
        r"\d+ 0 R", pdf.xref_get_key(font_xref, "DescendantFonts")[1]
    ).group()))
    cid_map = pdf.xref_stream(reference(pdf.xref_get_key(descendant, "CIDToGIDMap")))
    to_unicode = pdf.xref_stream(reference(pdf.xref_get_key(font_xref, "ToUnicode")))
    bfchar = to_unicode.split(b"beginbfchar", 1)[1].split(b"endbfchar", 1)[0]
    unicode_map = dict(
        (int(cid, 16), int(codepoint, 16))
        for cid, codepoint in BFCHAR.findall(bfchar)
    )
    assert unicode_map
    assert pdf.xref_get_key(font_xref, "Encoding") == (
        "name", "/Identity-V" if vertical else "/Identity-H"
    )
    cmap = source.getBestCmap()
    units = source["head"].unitsPerEm
    widths = [int(value) for value in re.findall(
        r"-?\d+", pdf.xref_get_key(descendant, "W")[1]
    )]
    assert widths.pop(0) == 1
    vertical_metrics = None
    if vertical:
        vertical_metrics = [int(value) for value in re.findall(
            r"-?\d+", pdf.xref_get_key(descendant, "W2")[1]
        )]
        assert vertical_metrics.pop(0) == 1
    for cid, codepoint in unicode_map.items():
        assert cid > 0 and 2 * cid + 2 <= len(cid_map)
        gid = int.from_bytes(cid_map[2 * cid : 2 * cid + 2], "big")
        glyph_name = cmap[codepoint]
        assert gid == source.getGlyphID(glyph_name), (cid, codepoint, gid)
        advance = source["hmtx"][glyph_name][0]
        assert widths[cid - 1] == round(1000 * advance / units)
        if vertical:
            assert vertical_metrics is not None
            vertical_advance, top_bearing = source["vmtx"][glyph_name]
            y_max = source["glyf"][glyph_name].yMax
            actual = vertical_metrics[3 * (cid - 1) : 3 * cid]
            expected = [
                round(-1000 * vertical_advance / units),
                round(500 * advance / units),
                round(1000 * (y_max + top_bearing) / units),
            ]
            assert actual == expected, (cid, actual, expected)
    return len(unicode_map)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pdf", type=Path)
    args = parser.parse_args()
    source = TTFont(SOURCE, checkChecksums=2)
    pdf = fitz.open(args.pdf)
    assert len(pdf) == 3
    horizontal = pdf[0].get_fonts(full=True)
    vertical = pdf[1].get_fonts(full=True)
    courier = pdf[2].get_fonts(full=True)
    assert len(horizontal) == len(vertical) == len(courier) == 1
    assert horizontal[0][5] == "Identity-H"
    assert vertical[0][5] == "Identity-V"
    assert courier[0][2:6] == ("Type1", "Courier", "F1", "WinAnsiEncoding")
    courier_chars = pdf[2].get_text("rawdict")["blocks"][0]["lines"][0]["spans"][0]["chars"]
    assert "".join(item["c"] for item in courier_chars) == "Courier: Café 123"
    for index, item in enumerate(courier_chars):
        assert isclose(item["origin"][0], 20 + index * 14.4, abs_tol=0.001)
        assert isclose(item["origin"][1], 50, abs_tol=0.001)
    h_count = mapped_metrics(pdf, source, horizontal[0][0], False)
    v_count = mapped_metrics(pdf, source, vertical[0][0], True)
    print(f"Identity-H: {h_count} Unicode/CID/GID/W entries checked")
    print(f"Identity-V: {v_count} Unicode/CID/GID/W/W2 entries checked")
    print("Courier/WinAnsi text and 600-unit AFM advances checked")


if __name__ == "__main__":
    main()
