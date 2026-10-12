#!/usr/bin/env python3
"""Independent fixed MuPDF sttext checker: XML [--cjk]. No assertions/-O bypass."""
import math
import copy
import argparse
import importlib.util
import json
import subprocess
from pathlib import Path
import sys
import tempfile
import xml.etree.ElementTree as ET


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check(path, cjk=False, announce=True, vertical_source=None):
    require(not vertical_source or cjk, "vertical fixture requires CJK")
    pages = ET.parse(path).getroot().findall("page")
    require(len(pages) == 3, "three fixture pages required")
    initial = "你你你" if cjk else "A A A"
    edited = "你好" if cjk else "A éÅ"
    expected = [initial * 2, edited + initial, edited + initial]
    # MuPDF's default stext inserts a visual space across the Latin compensation
    # gap. Keep that fixed extraction expectation explicit, not a general filter.
    if not cjk:
        expected[2] = edited + " " + initial
    characters = []
    for page, text in zip(pages, expected):
        if vertical_source:
            require(all(line.attrib.get("wmode") == "1" and line.attrib.get("dir") == "0 1"
                        for line in page.iter("line")), "vertical writing mode")
        chars = list(page.iter("char"))
        require("".join(char.attrib["c"] for char in chars) == text, "exact fixture text")
        fonts = list(page.iter("font"))
        require(bool(fonts), "fixture font binding")
        require(sum(len(list(font.iter("char"))) for font in fonts) == len(chars),
                "all characters must have a font binding")
        for char in chars:
            require(all(math.isfinite(float(char.attrib[axis])) for axis in ("x", "y")),
                    "finite positions")
        for font in fonts:
            require(font.attrib.get("name") == ("NotoSansSC-Thin" if cjk else "ArialMT"), "fixture font binding")
            require(float(font.attrib["size"]) == 24, "fixture font size")
        characters.append(chars)
    def point(char):
        value = (float(char.attrib["x"]), float(char.attrib["y"]))
        require(all(math.isfinite(coordinate) for coordinate in value), "finite positions")
        return value
    old, natural, compensated = characters
    prefix_length = 1 if cjk else 2
    for index in range(prefix_length):
        require(point(old[index]) == point(natural[index]) == point(compensated[index]), "retained prefix position")
    deltas = []
    for offset in range(len(initial)):
        baseline = point(old[len(initial) + offset])
        following = point(compensated[len(edited) + (0 if cjk else 1) + offset])
        moved = point(natural[len(edited) + offset])
        require(abs(baseline[0] - following[0]) < 0.01 and abs(baseline[1] - following[1]) < 0.01, "compensated following position")
        advance_axis = 1 if vertical_source else 0
        fixed_axis = 1 - advance_axis
        require(abs(baseline[fixed_axis] - moved[fixed_axis]) < 0.01, "natural following baseline")
        deltas.append(moved[advance_axis] - baseline[advance_axis])
    require(all(delta < -1 for delta in deltas), "shorter replacement must advance less naturally")
    require(max(deltas) - min(deltas) < 0.01, "consistent following translation")
    if vertical_source:
        from fontTools.ttLib import TTFont
        with TTFont(vertical_source) as source:
            cmap = source.getBestCmap()
            advance = lambda char: source["vmtx"].metrics[cmap[ord(char)]][0]
            expected_delta = (advance("好") - 2 * advance("你")) * 24 / source["head"].unitsPerEm
        require(all(abs(delta - expected_delta) < 0.01 for delta in deltas), "source vertical advance")
    if announce:
        print("PASS: exact three-page text/font/size, original prefix, compensated following positions and natural translation")


def self_test(path, cjk=False, vertical_source=None):
    """Each independently corrupted fixture must fail, including under python -O."""
    root = ET.parse(path).getroot()
    def corrupt_text(tree):
        next(tree.iter("char")).set("c", "X")
    def corrupt_page(tree):
        tree.remove(tree.findall("page")[-1])
    def corrupt_font(tree):
        next(tree.iter("font")).set("name", "WrongFont")
    def corrupt_size(tree):
        next(tree.iter("font")).set("size", "25")
    def corrupt_position(tree):
        chars = list(tree.findall("page")[2].iter("char"))
        char = chars[2 if cjk else 5]
        char.set("x", str(float(char.attrib["x"]) + 3))
    def corrupt_finite(tree):
        # The inserted glyph is not a retained-prefix/following-position sample.
        # It must nevertheless have finite coordinates.
        list(tree.findall("page")[1].iter("char"))[1 if cjk else 2].set("x", "nan")
    mutations = [(corrupt_text, "exact fixture text"),
                 (corrupt_page, "three fixture pages required"),
                 (corrupt_font, "fixture font binding"),
                 (corrupt_size, "fixture font size"),
                 (corrupt_position, "compensated following position"),
                 (corrupt_finite, "finite positions")]
    if vertical_source:
        def corrupt_mode(tree):
            next(tree.iter("line")).set("wmode", "0")
        def corrupt_advance(tree):
            for char in list(tree.findall("page")[1].iter("char"))[2:]:
                char.set("y", str(float(char.attrib["y"]) + 1))
        mutations.extend([(corrupt_mode, "vertical writing mode"),
                          (corrupt_advance, "source vertical advance")])
    with tempfile.TemporaryDirectory(prefix="rzpdf-subset-edit-check-") as directory:
        for mutation, expected_error in mutations:
            tree = copy.deepcopy(root)
            mutation(tree)
            candidate = directory + "/invalid.xml"
            ET.ElementTree(tree).write(candidate, encoding="utf-8")
            try:
                check(candidate, cjk, announce=False, vertical_source=vertical_source)
            except ValueError as error:
                require(str(error) == expected_error,
                        "negative control failed at wrong check: " + str(error))
                continue
            raise ValueError("checker accepted negative control: " + mutation.__name__)
    print("PASS: " + ("eight" if vertical_source else "six") + " independent negative controls rejected")


def check_vertical_pdf(pdf_path, source_path, tail_source=None):
    """Independently compare fixed PDF W/W2 dictionaries to full source metrics."""
    from fontTools.ttLib import TTFont
    document = json.loads(subprocess.check_output(["qpdf", "--json", str(pdf_path)]))
    objects = {key: value.get("value") for key, value in document["qpdf"][1].items()}
    with TTFont(source_path) as source:
        cmap = source.getBestCmap()
        scale = 1000 / source["head"].unitsPerEm
        expected = {}
        base = source["name"].getDebugName(6)
        for tag, repertoire in [("EDITAA", "你"), ("EDITAB", "你好"), ("EDITAC", "你好")]:
            widths, triples = [], []
            for char in sorted(set(repertoire)):
                glyph_name = cmap[ord(char)]
                width = source["hmtx"].metrics[glyph_name][0]
                advance, bearing = source["vmtx"].metrics[glyph_name]
                glyph = source["glyf"][glyph_name]
                y_max = getattr(glyph, "yMax", 0)
                widths.append(width * scale)
                triples.extend([-advance * scale, width * scale / 2, (y_max + bearing) * scale])
            expected["/" + tag + "+" + base] = (widths, triples)
    tail_identity = None
    if tail_source:
        with TTFont(tail_source) as tail:
            tail_identity = "/TAILAA+" + tail["name"].getDebugName(6)
            glyph = tail.getBestCmap()[ord("B")]
            expected[tail_identity] = ([tail["hmtx"].metrics[glyph][0] *
                                        1000 / tail["head"].unitsPerEm],)

    def verify(candidate):
        fonts = [value for value in candidate.values() if isinstance(value, dict)
                 and value.get("/Subtype") == "/Type0"]
        require(len(fonts) == len(expected) and {font.get("/BaseFont") for font in fonts} == set(expected),
                "vertical PDF font identities")
        for font in fonts:
            horizontal = font.get("/BaseFont") == tail_identity
            require(font.get("/Encoding") == ("/Identity-H" if horizontal else "/Identity-V"), "vertical PDF encoding")
            references = font.get("/DescendantFonts")
            require(isinstance(references, list) and len(references) == 1, "vertical PDF descendant")
            descendant = candidate.get("obj:" + references[0])
            require(isinstance(descendant, dict) and descendant.get("/Subtype") == "/CIDFontType2",
                    "vertical PDF descendant")
            require(not horizontal or "/W2" not in descendant, "horizontal tail has vertical metrics")
            for key, numbers in zip(("/W", "/W2"), expected[font["/BaseFont"]]):
                array = descendant.get(key)
                require(isinstance(array, list) and len(array) == 2 and array[0] == 1
                        and isinstance(array[1], list) and len(array[1]) == len(numbers),
                        "vertical PDF metric shape")
                require(all(isinstance(actual, (int, float)) and math.isfinite(actual)
                            and abs(actual - wanted) < 1e-7
                            for actual, wanted in zip(array[1], numbers)), "vertical PDF source metrics")
    verify(objects)
    for mutation, error in [("encoding", "vertical PDF encoding"),
                            ("advance", "vertical PDF source metrics"),
                            ("origin", "vertical PDF source metrics"),
                            ("count", "vertical PDF metric shape")]:
        candidate = copy.deepcopy(objects)
        font = next(value for value in candidate.values() if isinstance(value, dict)
                    and value.get("/Subtype") == "/Type0" and value.get("/BaseFont") != tail_identity)
        if mutation == "encoding":
            font["/Encoding"] = "/Identity-H"
        else:
            w2 = candidate["obj:" + font["/DescendantFonts"][0]]["/W2"][1]
            if mutation == "count":
                w2.extend([0, 0, 0])
            else:
                w2[0 if mutation == "advance" else 2] += 1
        try:
            verify(candidate)
        except ValueError as failure:
            require(str(failure) == error, "PDF metric negative failed at wrong check")
        else:
            raise ValueError("PDF metric negative incorrectly accepted")
    if tail_identity:
        for mutation, expected_error in [("encoding", "vertical PDF encoding"),
                                          ("width", "vertical PDF source metrics"),
                                          ("w2", "horizontal tail has vertical metrics")]:
            candidate = copy.deepcopy(objects)
            font = next(value for value in candidate.values() if isinstance(value, dict)
                        and value.get("/BaseFont") == tail_identity and value.get("/Subtype") == "/Type0")
            descendant = candidate["obj:" + font["/DescendantFonts"][0]]
            if mutation == "encoding":
                font["/Encoding"] = "/Identity-V"
            elif mutation == "width":
                descendant["/W"][1][0] += 1
            else:
                descendant["/W2"] = [1, [0, 0, 0]]
            try:
                verify(candidate)
            except ValueError as failure:
                require(str(failure) == expected_error, "tail PDF negative failed at wrong check")
            else:
                raise ValueError("tail PDF negative incorrectly accepted")
    print("PASS: PDF Identity-V/W/W2 and optional Identity-H tail match full sources; " +
          ("seven" if tail_identity else "four") + " metric negative controls")


def check_fonts(source, directory, cjk=False, *, repertoire_pair=None, tail_source=None):
    """Validate all three independently extracted fonts against the full source."""
    from fontTools.ttLib import TTFont
    spec = importlib.util.spec_from_file_location(
        "subset_font_checker", Path(__file__).with_name("check-cjk-subset.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    initial, edited = repertoire_pair or (("你你你", "你好") if cjk else ("A A A", "A éÅ"))
    base = "NotoSansSC-Thin" if cjk else "ArialMT"
    names = set()
    paths = sorted(Path(directory).glob("*.ttf"))
    require(len(paths) == (4 if tail_source else 3), "expected extracted fixture font count")
    for path in paths:
        with TTFont(path) as font:
            name = font["name"].getDebugName(6)
        require(isinstance(name, str), "subset identity required")
        tag = name.split("+")[0]
        if tag == "TAILAA" and tail_source:
            require(name not in names, "unique fixture subset identity")
            names.add(name)
            with TTFont(tail_source) as tail:
                tail_name = tail["name"].getDebugName(6)
            module.check(tail_source, path, None, repertoire="BBB",
                         postscript_name="TAILAA+" + tail_name)
            continue
        require(tag in ("EDITAA", "EDITAB", "EDITAC"), "fixture subset tag")
        require(name not in names, "unique fixture subset identity")
        names.add(name)
        module.check(source, path, None, latin=not cjk,
                     repertoire=initial if tag == "EDITAA" else edited,
                     postscript_name=tag + "+" + base)
    # Repair checksums after corrupting head: rejection must be semantic, not
    # just an incidental damaged-file/checksum error.
    path = paths[0]
    with TTFont(path) as font:
        name = font["name"].getDebugName(6)
    repertoire = initial if name.startswith("EDITAA+") else edited
    with tempfile.TemporaryDirectory(prefix="rzpdf-edit-font-negative-") as temporary:
        altered = Path(temporary) / "wrong-bounds.ttf"
        with TTFont(path, recalcBBoxes=False) as font:
            font["head"].xMin -= 1
            font.save(altered)
        altered_metrics = Path(temporary) / "wrong-preserved-hhea.ttf"
        with TTFont(path, recalcBBoxes=False) as font:
            font["hhea"].ascent -= 1
            font.save(altered_metrics)
        for candidate, expected_error in [
                (altered, "subset global outline bounds"),
                (altered_metrics, "preserved table bytes: hhea"),
                (source, "exact selected Unicode repertoire")]:
            try:
                module.check(source, candidate, None, latin=not cjk,
                             repertoire=repertoire, postscript_name=name)
            except ValueError as error:
                require(str(error) == expected_error,
                        "font negative failed at wrong check: " + str(error))
            else:
                raise ValueError("font negative incorrectly accepted: " + expected_error)
    print("PASS: three font negative controls rejected at the intended checks")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("xml")
    parser.add_argument("--cjk", action="store_true")
    parser.add_argument("--vertical", metavar="SOURCE_TTF")
    parser.add_argument("--pdf", metavar="PDF_PATH")
    parser.add_argument("--fonts", nargs=2, metavar=("SOURCE_TTF", "EXTRACT_DIRECTORY"))
    args = parser.parse_args()
    check(args.xml, args.cjk, vertical_source=args.vertical)
    self_test(args.xml, args.cjk, vertical_source=args.vertical)
    if args.pdf:
        require(args.vertical is not None, "PDF metric check requires --vertical source")
        check_vertical_pdf(args.pdf, args.vertical)
    if args.fonts:
        check_fonts(*args.fonts, cjk=args.cjk)
