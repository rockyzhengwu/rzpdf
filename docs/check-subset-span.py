#!/usr/bin/env python3
"""Check the fixed edit_subset_span MuPDF stext independently, including -O."""
import argparse
import copy
import math
import importlib.util
import tempfile
from pathlib import Path
import xml.etree.ElementTree as ET


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check(path, cjk=False, mixed=False, cid_tail=None, vertical=None):
    vertical_advance = None
    vertical_metrics = {}
    if vertical:
        require(cjk, "vertical span fixture requires --cjk")
        from fontTools.ttLib import TTFont
        with TTFont(vertical) as font:
            scale = 24 / font["head"].unitsPerEm
            for char in "你好":
                glyph = font.getBestCmap()[ord(char)]
                advance, bearing = font["vmtx"].metrics[glyph]
                width = font["hmtx"].metrics[glyph][0]
                y_max = getattr(font["glyf"][glyph], "yMax", 0)
                vertical_metrics[char] = (width * scale / 2, (y_max + bearing) * scale,
                                          advance * scale - 1)
            vertical_advance = vertical_metrics["你"][2]
    tail_name, tail_advance = "Courier", 12.8
    if cid_tail:
        from fontTools.ttLib import TTFont
        with TTFont(cid_tail) as font:
            tail_name = font["name"].getDebugName(6)
            glyph = font.getBestCmap()[ord("B")]
            tail_advance = font["hmtx"].metrics[glyph][0] / font["head"].unitsPerEm * 18 + 2
        mixed = True
    pages = ET.parse(path).getroot().findall("page")
    require(len(pages) == 3, "three pages required")
    initial = "你你你" if cjk else "AAA"
    first = "你好" if cjk else "AéÅ"
    tail = "BBB" if mixed else initial
    expected = [initial + tail + initial, first + tail[1:] + initial,
                first + tail[1:] + initial]
    characters = []
    for page, text in zip(pages, expected):
        chars = list(page.iter("char"))
        require("".join(char.attrib["c"] for char in chars) == text, "exact text")
        fonts = list(page.iter("font"))
        require(bool(fonts) and sum(len(list(font.iter("char"))) for font in fonts) == len(chars),
                "font ownership")
        for font in fonts:
            tail_font = mixed and all(char.attrib["c"] == "B" for char in font.iter("char"))
            require(font.attrib.get("name") == (tail_name if tail_font else
                    ("NotoSansSC-Thin" if cjk else "ArialMT")), "font name")
            require(float(font.attrib["size"]) == (18 if tail_font else 24), "font size")
        for char in chars:
            require(all(math.isfinite(float(char.attrib[axis])) for axis in ("x", "y")), "finite position")
        if vertical:
            for line in page.iter("line"):
                horizontal_tail = mixed and all(char.attrib["c"] == "B" for char in line.iter("char"))
                require(line.attrib.get("wmode") == ("0" if horizontal_tail else "1") and
                        all(abs(float(value) - expected) < 0.001 for value, expected in
                            zip(line.attrib.get("dir", "").split(), (1, 0) if horizontal_tail else (0, 1))) and
                        len(line.attrib.get("dir", "").split()) == 2,
                        "vertical writing direction")
        characters.append(chars)
    def point(char):
        return float(char.attrib["x"]), float(char.attrib["y"])
    def same(a, b, label):
        require(all(abs(x - y) < 0.01 for x, y in zip(point(a), point(b))), label)
    old, natural, compensated = characters
    same(old[0], natural[0], "prefix position")
    same(old[0], compensated[0], "prefix position")
    same(old[1], natural[1], "insertion position")
    same(old[1], compensated[1], "insertion position")
    shifts = []
    for offset in range(2):
        baseline, moved, held = old[4 + offset], natural[len(first) + offset], compensated[len(first) + offset]
        same(baseline, held, "compensated suffix")
        axis = 1 if vertical and not mixed else 0
        require(abs(point(baseline)[1 - axis] - point(moved)[1 - axis]) < 0.01, "suffix baseline")
        shifts.append(point(moved)[axis] - point(baseline)[axis])
    require(all(shift < -1 for shift in shifts) and abs(shifts[0] - shifts[1]) < 0.01,
            "natural suffix translation")
    if mixed:
        require(all(abs(shift + tail_advance) < 0.01 for shift in shifts), "tail font advance")
    if vertical and not mixed:
        require(all(abs(shift + vertical_advance) < 0.01 for shift in shifts), "source vertical advance")
    for offset in range(3):
        same(old[6 + offset], natural[len(first) + 2 + offset], "untouched invocation")
        same(old[6 + offset], compensated[len(first) + 2 + offset], "untouched invocation")
    if vertical:
        layouts = [[("你你你", 0, 0), (tail, 80, 0), ("你你你", 160, 0)],
                   [("你好", 0, 0), (tail[1:], 80, 0), ("你你你", 160, 0)],
                   [("你好", 0, 0), (tail[1:], 80, tail_advance if mixed else vertical_advance), ("你你你", 160, 0)]]
        for chars, columns in zip(characters, layouts):
            expected_points = []
            for text, column, advance in columns:
                for char in text:
                    if mixed and char == "B":
                        expected_points.append((48 + column + advance, 842 - 760))
                        advance += tail_advance
                        continue
                    origin_x, origin_y, step = vertical_metrics[char]
                    # MuPDF stext reports the vertical character's lower em
                    # anchor (quad bottom), one 24pt em below its glyph origin.
                    expected_points.append((48 + column - origin_x, 842 - 760 + advance + origin_y + 24))
                    advance += step
            require(len(chars) == len(expected_points) and all(
                abs(actual - wanted) < 0.01
                for char, expected_point in zip(chars, expected_points)
                for actual, wanted in zip(point(char), expected_point)), "absolute vertical layout")


def self_test(path, cjk=False, mixed=False, cid_tail=None, vertical=None):
    mixed = mixed or cid_tail is not None
    root = ET.parse(path).getroot()
    def character(tree, page, index):
        return list(tree.findall("page")[page].iter("char"))[index]
    def text(tree):
        character(tree, 0, 0).set("c", "X")
    def page(tree):
        tree.remove(tree.findall("page")[-1])
    def font(tree):
        next(tree.iter("font")).set("name", "WrongFont")
    def size(tree):
        next(tree.iter("font")).set("size", "25")
    def suffix(tree):
        char = character(tree, 2, 2 if cjk else 3)
        char.set("x", str(float(char.attrib["x"]) + 3))
    def untouched(tree):
        char = character(tree, 1, 4 if cjk else 5)
        char.set("y", str(float(char.attrib["y"]) + 3))
    def finite(tree):
        character(tree, 1, 1).set("x", "nan")
    mutations = [(text, "exact text"), (page, "three pages required"),
                 (font, "font name"), (size, "font size"),
                 (suffix, "compensated suffix"), (untouched, "untouched invocation"),
                 (finite, "finite position")]
    if vertical:
        def direction(tree):
            next(tree.iter("line")).set("dir", "1 0")
        def writing_mode(tree):
            next(tree.iter("line")).set("wmode", "0")
        def advance(tree):
            for index in (2 if cjk else 3, 3 if cjk else 4):
                char = character(tree, 1, index)
                char.set("y", str(float(char.attrib["y"]) - 1))
        mutations.extend([(direction, "vertical writing direction"),
                          (writing_mode, "vertical writing direction")])
        if not mixed:
            mutations.append((advance, "source vertical advance"))
        def translation(tree):
            for char in tree.iter("char"):
                char.set("x", str(float(char.attrib["x"]) + 5))
                char.set("y", str(float(char.attrib["y"]) + 7))
        mutations.append((translation, "absolute vertical layout"))
    if mixed:
        if vertical:
            def tail_direction(tree):
                next(line for line in tree.iter("line")
                     if all(char.attrib["c"] == "B" for char in line.iter("char"))).set("dir", "0 1")
            def tail_writing_mode(tree):
                next(line for line in tree.iter("line")
                     if all(char.attrib["c"] == "B" for char in line.iter("char"))).set("wmode", "1")
            mutations.extend([(tail_direction, "vertical writing direction"),
                              (tail_writing_mode, "vertical writing direction")])
        def tail_font(tree):
            next(font for font in tree.iter("font")
                 if all(char.attrib["c"] == "B" for char in font.iter("char"))).set("name", "WrongTail")
        def tail_size(tree):
            next(font for font in tree.iter("font")
                 if all(char.attrib["c"] == "B" for char in font.iter("char"))).set("size", "24")
        def tail_advance(tree):
            for index in (2 if cjk else 3, 3 if cjk else 4):
                char = character(tree, 1, index)
                char.set("x", str(float(char.attrib["x"]) + 1))
        mutations.append((tail_advance, "tail font advance"))
        mutations.extend([(tail_font, "font name"), (tail_size, "font size")])
    with tempfile.TemporaryDirectory(prefix="rzpdf-subset-span-check-") as directory:
        candidate = Path(directory) / "invalid.xml"
        for mutate, expected in mutations:
            tree = copy.deepcopy(root)
            mutate(tree)
            ET.ElementTree(tree).write(candidate, encoding="utf-8")
            try:
                check(candidate, cjk, mixed, cid_tail, vertical)
            except ValueError as error:
                require(str(error) == expected, "negative control failed at wrong check: " + str(error))
            else:
                raise ValueError("accepted negative control: " + mutate.__name__)


def check_reverse(path, primary_source, tail_source):
    """Fixed Arial horizontal -> NotoSC vertical fixture, no inferred advances."""
    from fontTools.ttLib import TTFont
    with TTFont(primary_source) as font:
        cmap = font.getBestCmap()
        steps = {char: font["hmtx"].metrics[cmap[ord(char)]][0] *
                 24 / font["head"].unitsPerEm + 1 for char in "AéÅ"}
        primary_name = font["name"].getDebugName(6)
    with TTFont(tail_source) as font:
        glyph = font.getBestCmap()[ord("B")]
        scale = 18 / font["head"].unitsPerEm
        advance, bearing = font["vmtx"].metrics[glyph]
        origin_x = font["hmtx"].metrics[glyph][0] * scale / 2
        origin_y = (getattr(font["glyf"][glyph], "yMax", 0) + bearing) * scale
        tail_step = advance * scale - 2
        tail_name = font["name"].getDebugName(6)
    pages = ET.parse(path).getroot().findall("page")
    require(len(pages) == 3, "three pages required")
    for page_index, page in enumerate(pages):
        first = "AAA" if page_index == 0 else "AéÅ"
        tail = "BBB" if page_index == 0 else "BB"
        chars = list(page.iter("char"))
        require("".join(char.attrib["c"] for char in chars) == first + tail + "AAA", "exact reverse text")
        require(sum(len(list(font.iter("char"))) for font in page.iter("font")) == len(chars)
                and sum(len(list(line.iter("char"))) for line in page.iter("line")) == len(chars),
                "reverse font ownership")
        for line in page.iter("line"):
            is_tail = all(char.attrib["c"] == "B" for char in line.iter("char"))
            require(line.attrib.get("wmode") == ("1" if is_tail else "0") and
                    line.attrib.get("dir") == ("0 1" if is_tail else "1 0"), "reverse direction")
            for font in line.iter("font"):
                require(font.attrib.get("name") == (tail_name if is_tail else primary_name), "reverse font")
                require(float(font.attrib["size"]) == (18 if is_tail else 24), "reverse size")
        points = []
        for text, x in [(first, 48), ("AAA", 208)]:
            column = []
            for char in text:
                column.append((x, 82))
                x += steps[char]
            if text == first and not points:
                points.extend(column)
                start = tail_step if page_index == 2 else 0
                points.extend((128 - origin_x, 82 + origin_y + 18 + start + index * tail_step)
                              for index in range(len(tail)))
            else:
                points.extend(column)
        require(len(chars) == len(points) and all(
            math.isfinite(float(char.attrib[axis])) and abs(float(char.attrib[axis]) - wanted) < 0.01
            for char, point in zip(chars, points) for axis, wanted in zip(("x", "y"), point)),
            "absolute reverse layout")


def check_reverse_pdf(path, primary_source, tail_source):
    import json
    import subprocess
    from fontTools.ttLib import TTFont
    document = json.loads(subprocess.check_output(["qpdf", "--json", str(path)]))
    objects = {key: value.get("value") for key, value in document["qpdf"][1].items()}
    expected = {}
    for source, entries, vertical in [
        (primary_source, [("EDITAA", "A"), ("EDITAB", "AéÅ"), ("EDITAC", "AéÅ")], False),
        (tail_source, [("TAILAA", "B")], True)]:
        with TTFont(source) as font:
            scale = 1000 / font["head"].unitsPerEm
            for tag, text in entries:
                widths, triples = [], []
                for char in sorted(set(text)):
                    glyph = font.getBestCmap()[ord(char)]
                    width = font["hmtx"].metrics[glyph][0]
                    widths.append(width * scale)
                    if vertical:
                        advance, bearing = font["vmtx"].metrics[glyph]
                        triples.extend([-advance * scale, width * scale / 2,
                                        (getattr(font["glyf"][glyph], "yMax", 0) + bearing) * scale])
                expected["/" + tag + "+" + font["name"].getDebugName(6)] = (vertical, widths, triples)
    def verify(candidate):
        fonts = [obj for obj in candidate.values() if isinstance(obj, dict) and obj.get("/Subtype") == "/Type0"]
        require(len(fonts) == 4 and {font.get("/BaseFont") for font in fonts} == set(expected), "reverse PDF identities")
        for font in fonts:
            vertical, widths, triples = expected[font["/BaseFont"]]
            require(font.get("/Encoding") == ("/Identity-V" if vertical else "/Identity-H"), "reverse PDF encoding")
            references = font.get("/DescendantFonts")
            require(isinstance(references, list) and len(references) == 1, "reverse PDF descendant")
            descendant = candidate.get("obj:" + references[0])
            require(isinstance(descendant, dict) and descendant.get("/Subtype") == "/CIDFontType2", "reverse PDF descendant")
            require(vertical or "/W2" not in descendant, "reverse horizontal W2")
            for key, values in [("/W", widths)] + ([("/W2", triples)] if vertical else []):
                array = descendant.get(key)
                require(isinstance(array, list) and len(array) == 2 and array[0] == 1 and
                        isinstance(array[1], list) and len(array[1]) == len(values), "reverse PDF shape")
                require(all(isinstance(actual, (int, float)) and math.isfinite(actual) and abs(actual - wanted) < 1e-7
                            for actual, wanted in zip(array[1], values)), "reverse PDF source metrics")
    verify(objects)
    for vertical, mutation, error in [
        (False, "encoding", "reverse PDF encoding"), (True, "encoding", "reverse PDF encoding"),
        (False, "width", "reverse PDF source metrics"), (True, "width", "reverse PDF source metrics"),
        (True, "advance", "reverse PDF source metrics"), (True, "origin", "reverse PDF source metrics"),
        (True, "count", "reverse PDF shape"), (False, "w2", "reverse horizontal W2")]:
        candidate = copy.deepcopy(objects)
        font = next(obj for obj in candidate.values() if isinstance(obj, dict) and obj.get("/Subtype") == "/Type0"
                    and expected[obj["/BaseFont"]][0] == vertical)
        descendant = candidate["obj:" + font["/DescendantFonts"][0]]
        if mutation == "encoding":
            font["/Encoding"] = "/Identity-H" if vertical else "/Identity-V"
        elif mutation == "w2":
            descendant["/W2"] = [1, [0, 0, 0]]
        elif mutation == "count":
            descendant["/W2"][1].extend([0, 0, 0])
        else:
            descendant["/W" if mutation == "width" else "/W2"][1][2 if mutation == "origin" else 0] += 1
        try:
            verify(candidate)
        except ValueError as failure:
            require(str(failure) == error, "reverse PDF negative failed at wrong check")
        else:
            raise ValueError("reverse PDF negative incorrectly accepted")
    print("PASS: reverse Identity-H/W and Identity-V/W/W2 source metrics; eight dictionary negative controls")


def self_test_reverse(path, primary_source, tail_source):
    root = ET.parse(path).getroot()
    mutations = []
    def set_character(tree, page, index, attribute, value):
        list(tree.findall("page")[page].iter("char"))[index].set(attribute, value)
    mutations.append((lambda tree: set_character(tree, 0, 0, "c", "X"), "exact reverse text"))
    mutations.append((lambda tree: tree.remove(tree.findall("page")[-1]), "three pages required"))
    for tail in (False, True):
        def selected_line(tree, tail=tail):
            return next(line for line in tree.iter("line")
                        if all(char.attrib["c"] == "B" for char in line.iter("char")) == tail)
        mutations.extend([
            (lambda tree, select=selected_line, tail=tail: select(tree).set("wmode", "0" if tail else "1"),
             "reverse direction"),
            (lambda tree, select=selected_line, tail=tail: select(tree).set("dir", "1 0" if tail else "0 1"),
             "reverse direction"),
            (lambda tree, select=selected_line: next(select(tree).iter("font")).set("name", "Wrong"),
             "reverse font"),
            (lambda tree, select=selected_line: next(select(tree).iter("font")).set("size", "25"),
             "reverse size")])
    mutations.extend([
        (lambda tree: set_character(tree, 2, 3, "y", "0"), "absolute reverse layout"),
        (lambda tree: set_character(tree, 1, 5, "x", "0"), "absolute reverse layout"),
        (lambda tree: set_character(tree, 0, 0, "x", "nan"), "absolute reverse layout")])
    def translation(tree):
        for char in tree.iter("char"):
            char.set("x", str(float(char.attrib["x"]) + 5))
            char.set("y", str(float(char.attrib["y"]) + 7))
    mutations.append((translation, "absolute reverse layout"))
    with tempfile.TemporaryDirectory(prefix="rzpdf-reverse-negative-") as directory:
        candidate = Path(directory) / "invalid.xml"
        for mutate, expected in mutations:
            tree = copy.deepcopy(root)
            mutate(tree)
            ET.ElementTree(tree).write(candidate, encoding="utf-8")
            try:
                check_reverse(candidate, primary_source, tail_source)
            except ValueError as error:
                require(str(error) == expected, "reverse negative failed at wrong check: " + str(error))
            else:
                raise ValueError("reverse negative incorrectly accepted: " + expected)
    print(f"PASS: {len(mutations)} reverse negative controls")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("xml")
    parser.add_argument("--reverse", nargs=2, metavar=("PRIMARY_TTF", "TAIL_TTF"))
    parser.add_argument("--cjk", action="store_true")
    parser.add_argument("--mixed-tail", action="store_true")
    parser.add_argument("--cid-tail", metavar="SOURCE_TTF")
    parser.add_argument("--vertical", metavar="SOURCE_TTF")
    parser.add_argument("--pdf", metavar="PDF_PATH")
    parser.add_argument("--fonts", nargs=2, metavar=("SOURCE_TTF", "EXTRACT_DIRECTORY"))
    args = parser.parse_args()
    if args.reverse:
        require(not any((args.cjk, args.mixed_tail, args.cid_tail, args.vertical)),
                "--reverse is a standalone fixed coordinate check")
        check_reverse(args.xml, *args.reverse)
        self_test_reverse(args.xml, *args.reverse)
        if args.pdf:
            check_reverse_pdf(args.pdf, *args.reverse)
        if args.fonts:
            require(Path(args.fonts[0]).resolve() == Path(args.reverse[0]).resolve(),
                    "reverse font source must match primary source")
            spec = importlib.util.spec_from_file_location(
                "subset_edit_checker", Path(__file__).with_name("check-subset-edit.py"))
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            module.check_fonts(*args.fonts, repertoire_pair=("AAA", "AéÅ"), tail_source=args.reverse[1])
        print("PASS: reverse mixed-mode exact text/fonts/directions and source absolute coordinates")
        raise SystemExit(0)
    require(not args.pdf or args.vertical, "--pdf requires --vertical SOURCE_TTF")
    check(args.xml, args.cjk, args.mixed_tail, args.cid_tail, args.vertical)
    self_test(args.xml, args.cjk, args.mixed_tail, args.cid_tail, args.vertical)
    if args.fonts or args.pdf:
        spec = importlib.util.spec_from_file_location(
            "subset_edit_checker", Path(__file__).with_name("check-subset-edit.py"))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    if args.pdf:
        require(args.cjk, "vertical PDF dictionary fixture requires --cjk")
        module.check_vertical_pdf(args.pdf, args.vertical, args.cid_tail)
    if args.fonts:
        module.check_fonts(*args.fonts, cjk=args.cjk,
                           repertoire_pair=("你你你", "你好") if args.cjk else ("AAA", "AéÅ"),
                           tail_source=args.cid_tail)
    print("PASS: cross-call exact text/font/size/positions and " +
          ("fifteen" if args.vertical and (args.mixed_tail or args.cid_tail) else
           "eleven" if args.vertical else "ten" if args.mixed_tail or args.cid_tail else "seven") + " negative controls")
