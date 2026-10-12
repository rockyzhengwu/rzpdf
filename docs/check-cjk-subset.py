#!/usr/bin/env python3
"""Read-only fixed create_cjk --subset fixture verifier; requires FontTools.

Usage: python3.11 docs/check-cjk-subset.py SOURCE.ttf EXTRACTED.ttf STEXT.xml
Append --self-test for fixed negative controls in an isolated temporary directory.
Append --latin for the fixed create_subset Arial composite fixture.
MuPDF extracts the FontFile2 and produces stext independently of rzpdf.
"""
import pathlib
import struct
import sys
import tempfile
import xml.etree.ElementTree as ET
from fontTools.ttLib import TTFont

LINES = ["rzpdf - 中文字体嵌入", "你好，世界！Hello PDF",
         "第二页 - 同一字体跨页复用", "中文可显示，也应正确提取。"]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def checksum(data):
    data += b"\0" * (-len(data) % 4)
    return sum(word[0] for word in struct.iter_unpack(">I", data)) & 0xffffffff


def check(source_path, subset_path, xml_path, latin=False, *, repertoire=None,
          postscript_name=None):
    fixture_lines = ["Composite subset - é Å ö ñ ç é"] if latin else LINES
    font_name = "ArialMT" if latin else "NotoSansSC-Thin"
    subset_name = "RZPDFY+ArialMT" if latin else "RZPDFX+NotoSansSC-Thin"
    if repertoire is not None:
        fixture_lines = [repertoire]
        require(postscript_name is not None, "explicit subset identity required")
        subset_name = postscript_name
    source = TTFont(source_path)
    subset = TTFont(subset_path)
    payload = pathlib.Path(subset_path).read_bytes()
    require(checksum(payload) == 0xB1B0AFBA, "whole-font checksum")
    count = struct.unpack_from(">H", payload, 4)[0]
    tags = []
    for index in range(count):
        tag, expected, offset, length = struct.unpack_from(">4sIII", payload, 12 + index * 16)
        require(offset % 4 == 0 and offset + length <= len(payload), "table range/alignment")
        data = payload[offset:offset + length]
        if tag == b"head":
            data = data[:8] + b"\0" * 4 + data[12:]
        require(checksum(data) == expected, "table checksum: " + repr(tag))
        tags.append(tag)
    require(tags == sorted(set(tags)), "directory ordering/duplicates")
    # Read original table bytes through FontTools' reader, not a recompiled
    # table: semantic decompilation can hide altered hinting or unused metrics.
    for tag in ("hhea", "hmtx", "maxp", "OS/2", "post", "cvt ", "fpgm",
                "prep", "gasp", "vhea", "vmtx", "STAT"):
        require((tag in source.reader) == (tag in subset.reader),
                "preserved table presence: " + tag)
        if tag in source.reader:
            require(source.reader[tag] == subset.reader[tag],
                    "preserved table bytes: " + tag)
    selected = {ord(character) for character in "".join(fixture_lines)}
    src_map = source.getBestCmap()
    sub_map = subset.getBestCmap()
    require(set(sub_map) == selected, "exact selected Unicode repertoire")
    require(source["maxp"].numGlyphs == subset["maxp"].numGlyphs, "original glyph count")
    require(subset["head"].indexToLocFormat == 1, "long loca")
    retained = {0}
    for scalar in selected:
        gid = source.getGlyphID(src_map[scalar])
        require(subset.getGlyphID(sub_map[scalar]) == gid, "original GID mapping")
        retained.add(gid)
    pending = list(retained)
    composite_count = sum(source["glyf"][src_map[scalar]].isComposite() for scalar in selected)
    if latin and repertoire is None:
        require(composite_count >= 5, "real composite coverage")
    while pending:
        gid = pending.pop()
        glyph = source["glyf"][source.getGlyphName(gid)]
        if glyph.isComposite():
            for component in glyph.components:
                child = source.getGlyphID(component.glyphName)
                if child not in retained:
                    retained.add(child)
                    pending.append(child)
    outline_bounds = []
    for gid in range(source["maxp"].numGlyphs):
        old_name, new_name = source.getGlyphName(gid), subset.getGlyphName(gid)
        old, new = source["glyf"][old_name], subset["glyf"][new_name]
        if gid not in retained:
            require(new.numberOfContours == 0, "unselected glyph not empty")
            continue
        require(old.getCoordinates(source["glyf"]) == new.getCoordinates(subset["glyf"]), "outline mismatch")
        # Expanded contour endpoints independently exclude empty composites.
        if new.getCoordinates(subset["glyf"])[1]:
            outline_bounds.append((new.xMin, new.yMin, new.xMax, new.yMax))
        require(old.isComposite() == new.isComposite(), "composite kind changed")
        if old.isComposite():
            require([source.getGlyphID(c.glyphName) for c in old.components] ==
                    [subset.getGlyphID(c.glyphName) for c in new.components], "component GID references")
        require(source["hmtx"].metrics[old_name] == subset["hmtx"].metrics[new_name], "horizontal metrics")
        if "vmtx" in source:
            require(source["vmtx"].metrics[old_name] == subset["vmtx"].metrics[new_name], "vertical metrics")
        def instructions(glyph):
            program = getattr(glyph, "program", None)
            return program.getBytecode() if program is not None else b""
        require(instructions(old) == instructions(new), "glyph instructions")
    expected_bounds = (min(b[0] for b in outline_bounds), min(b[1] for b in outline_bounds),
                       max(b[2] for b in outline_bounds), max(b[3] for b in outline_bounds)) if outline_bounds else (0, 0, 0, 0)
    head = subset["head"]
    require((head.xMin, head.yMin, head.xMax, head.yMax) == expected_bounds,
            "subset global outline bounds")
    def notices(font):
        return [(n.platformID, n.platEncID, n.langID, n.nameID, n.string)
                for n in font["name"].names if n.nameID in (0, 7, 13, 14)]
    require(notices(source) == notices(subset), "copyright/license notices changed")
    require(subset["name"].getDebugName(6) == subset_name, "subset PostScript identity")
    require(source["OS/2"].fsType == subset["OS/2"].fsType, "embedding flags changed")
    for tag in ("BASE", "GDEF", "GPOS", "GSUB", "DSIG", "JSTF", "kern", "LTSH", "hdmx", "VDMX", "PCLT"):
        require(tag not in subset, "stale omitted table: " + tag)
    if repertoire is not None:
        print(f"PASS font: {len(selected)} scalars, {len(retained)} retained GIDs, {composite_count} selected composites; checksums, outlines, metrics, bounds and notices")
        source.close()
        subset.close()
        return
    pages = ET.parse(xml_path).getroot().findall("page")
    require(len(pages) == (1 if latin else 2), "page count")
    for index, page in enumerate(pages):
        line_nodes = list(page.iter("line"))
        lines = ["".join(char.attrib["c"] for char in line.iter("char")) for line in line_nodes]
        require(lines == (fixture_lines if latin else fixture_lines[index * 2:index * 2 + 2]), "exact extracted page text")
        for line_index, line in enumerate(line_nodes):
            fonts = list(line.iter("font"))
            require(bool(fonts), "missing extracted font")
            for font in fonts:
                require(font.attrib.get("name") == font_name, "extracted font binding")
                require(float(font.attrib["size"]) == (24 if line_index == 0 else 16), "extracted font size")
    print(f"PASS: {len(selected)} scalars, {len(retained)} retained GIDs, {composite_count} selected composites; checksums, outlines, metrics, notices and exact page text")


def self_test(source_path, subset_path, xml_path, latin=False):
    # First establish a positive control. Catch only the exact expected error,
    # never dependency errors or arbitrary failures as a successful rejection.
    check(source_path, subset_path, xml_path, latin)
    def rejects(font_path, text_path, expected):
        try:
            check(source_path, font_path, text_path, latin)
        except ValueError as error:
            require(str(error) == expected, "negative failed at wrong check: " + str(error))
        else:
            raise ValueError("negative incorrectly accepted: " + expected)
        print("REJECT:", expected)
    with tempfile.TemporaryDirectory(prefix="rzpdf-subset-check-") as temporary:
        root = pathlib.Path(temporary)
        corrupt = bytearray(pathlib.Path(subset_path).read_bytes())
        corrupt[-1] ^= 1
        corrupt_path = root / "bad-checksum.ttf"
        corrupt_path.write_bytes(corrupt)
        rejects(corrupt_path, xml_path, "whole-font checksum")
        rejects(source_path, xml_path, "exact selected Unicode repertoire")
        stale_bounds = TTFont(subset_path, recalcBBoxes=False)
        stale_bounds["head"].xMin -= 1
        stale_path = root / "bad-global-bounds.ttf"
        # Saving repairs checksums, so this control must reach the bounds check.
        stale_bounds.save(stale_path)
        stale_bounds.close()
        rejects(stale_path, xml_path, "subset global outline bounds")
        for mode, expected in [("text", "exact extracted page text"),
                               ("pages", "page count"),
                               ("font", "extracted font binding"),
                               ("size", "extracted font size")]:
            tree = ET.parse(xml_path)
            document = tree.getroot()
            if mode == "text": next(document.iter("char")).set("c", "X")
            elif mode == "pages": document.remove(document.findall("page")[-1])
            elif mode == "font": next(document.iter("font")).set("name", "Helvetica")
            else: next(document.iter("font")).set("size", "12")
            path = root / (mode + ".xml")
            tree.write(path, encoding="utf-8")
            rejects(subset_path, path, expected)
    print("PASS: seven deterministic negative controls")


if __name__ == "__main__":
    flags = sys.argv[4:]
    if len(sys.argv) < 4 or any(flag not in ("--latin", "--self-test") for flag in flags) or len(set(flags)) != len(flags):
        raise SystemExit(__doc__)
    runner = self_test if "--self-test" in flags else check
    runner(*sys.argv[1:4], latin="--latin" in flags)
