"""Validate MuPDF stext XML for the fixed compare_cjk_span example."""
import argparse
import copy
import math
from pathlib import Path
import re
import xml.etree.ElementTree as ET

def require(condition, message):
    # Unlike assert, validation must also run under python -O.
    if not condition:
        raise ValueError(message)


def check(root, partial=False, same_chain=False, mixed_font=False):
    pages = root.findall("page")
    require(len(pages) == 4, "expected four comparison pages")
    chars = [page.findall(".//char") for page in pages]
    if mixed_font:
        fonts = pages[0].findall(".//font")
        require(len(fonts) == 3, "expected three fixed baseline font runs")
        for font, text, size, latin in zip(fonts,
                ["你好，世界！Hello PDF PDF", "Hello PDF", "你好，世界！Hello PDF PDF"],
                [24, 18, 24], [False, True, False]):
            name = font.get("name", "")
            require(name == "Helvetica" if latin else name.startswith("NotoSansSC"),
                    "baseline font binding mismatch")
            require(float(font.get("size", "nan")) == size, "baseline font size mismatch")
            require("".join(ch.get("c", "") for ch in font.findall(".//char")) == text,
                    "font run text mismatch")
    edited = "你好！Hello PDF PDF" if partial else "你好！ PDF"
    retained = len("Hello PDF PDF") if partial else 3
    expected = [
        ("你好，世界！Hello PDF PDFHello PDF你好，世界！Hello PDF PDF"
         if mixed_font else "你好，世界！Hello PDF PDF" * (3 if same_chain else 2)),
        "第二页 - 同一字体跨页复用中文可显示，也应正确提取。",
        edited,
        edited,
    ]
    for items, text in zip(chars, expected):
        require("".join(ch.get("c", "") for ch in items) == text, "text mismatch")
    for old, adjusted, ordinary in zip(
        chars[0][-retained:], chars[2][-retained:], chars[3][-retained:]
    ):
        for axis in ["x", "y"]:
            values = [float(ch.attrib[axis]) for ch in [old, adjusted, ordinary]]
            require(all(math.isfinite(value) for value in values), "nonfinite coordinate")
            require(abs(values[0] - values[1]) <= 0.002, "compensated suffix moved")
            if axis == "x":
                require(values[0] - values[2] >= 10, "ordinary control did not move left")
            else:
                require(abs(values[0] - values[2]) <= 0.002, "ordinary baseline moved")


def check_tj_qdf(data):
    # Fixed qpdf --qdf fixture syntax only, not a general PDF parser.
    arrays = re.findall(rb"(?m)^[^\r\n]*\[([^\r\n]*)\] TJ[^\r\n]*$", data)
    adjusted_arrays = 0
    for array in arrays:
        numbers_only = re.sub(rb"<[0-9a-fA-F]*>", b" ", array)
        numbers = [float(token) for token in numbers_only.split()]
        require(all(math.isfinite(number) for number in numbers), "nonfinite TJ number")
        # Compensation can also turn the independent trailing Tj into a TJ.
        # It has no original -125/75 pair and is checked via XML coordinates.
        if -125 not in numbers and 75 not in numbers:
            continue
        adjusted_arrays += 1
        require(numbers.count(-125) == 1 and numbers.count(75) == 1,
                "original TJ adjustments changed")
        require(numbers[-1] == 75 and numbers.index(-125) < numbers.index(75),
                "TJ adjustment order changed")
    require(adjusted_arrays == 5, "expected original and four rebuilt adjusted arrays")


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("xml", help="MuPDF stext XML from compare_cjk_span")
parser.add_argument("--self-test", action="store_true")
parser.add_argument("--tj-qdf", help="qpdf --qdf output of the same --tj/--tj-inside fixture")
mode = parser.add_mutually_exclusive_group()
mode.add_argument("--partial", action="store_true", help="validate retained end-event suffix")
mode.add_argument("--same-chain", action="store_true", help="validate parent endpoints enclosing child text")
mode.add_argument("--mixed-font", action="store_true", help="validate CJK parent and Helvetica child")
args = parser.parse_args()
if args.tj_qdf and not args.partial:
    parser.error("--tj-qdf requires --partial for the fixed retained suffix")
root = ET.parse(args.xml).getroot()
check(root, args.partial, args.same_chain, args.mixed_font)
if args.tj_qdf:
    qdf = Path(args.tj_qdf).read_bytes()
    check_tj_qdf(qdf)
    if args.self_test:
        for broken in [qdf.replace(b" -125 ", b" -124 ", 1),
                       qdf.replace(b" 75] TJ", b" 74] TJ", 1),
                       qdf.replace(b"] TJ", b"] Tj", 1)]:
            require(broken != qdf, "QDF mutation did not reach fixture")
            try:
                check_tj_qdf(broken)
            except ValueError:
                continue
            raise ValueError("QDF negative accepted")
        print("3 QDF adjustment negatives rejected")
    print("fixed TJ original adjustments passed")
if args.self_test:
    mutations = ["swap", "coordinate", "text", "missing-page"]
    if args.mixed_font:
        mutations.extend(["child-font", "child-size", "parent-font"])
    for mutation in mutations:
        broken = copy.deepcopy(root)
        pages = broken.findall("page")
        if mutation == "swap":
            broken.remove(pages[2])
            broken.append(pages[2])
        elif mutation == "coordinate":
            pages[2].findall(".//char")[-1].set("x", "0")
        elif mutation == "text":
            pages[2].findall(".//char")[0].set("c", "错")
        elif mutation == "child-font":
            pages[0].findall(".//font")[1].set("name", "NotoSansSC-Thin")
        elif mutation == "child-size":
            pages[0].findall(".//font")[1].set("size", "24")
        elif mutation == "parent-font":
            pages[0].findall(".//font")[0].set("name", "Helvetica")
        else:
            broken.remove(pages[1])
        try:
            check(broken, args.partial, args.same_chain, args.mixed_font)
        except ValueError:
            continue
        raise ValueError(f"negative accepted: {mutation}")
    print(f"{len(mutations)} negative cases rejected")
print("fixed cross-call text and independent suffix coordinates passed")
