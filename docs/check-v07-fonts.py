"""Independent v0.7 font mapping and positioned-run acceptance gate.

Build v07_font_roundtrip and render_document first. Run with Python 3.11,
FontTools, pikepdf, PyMuPDF, Pillow, mutool and qpdf. No Poppler or fuzz.
Fresh evidence directories retain logs, reference PDFs, rasters and report.
"""
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
import io
import fitz
import pikepdf
from PIL import Image, ImageChops, ImageStat
from fontTools.ttLib import TTFont
from fontTools.cffLib import CFFFontSet
from fontTools.pens.recordingPen import RecordingPen

ROOT = Path(__file__).resolve().parents[1]
FIX = ROOT / "pdf/tests/fixtures/v07"
OUT = Path(tempfile.mkdtemp(prefix="v07-fonts-", dir=ROOT / "tmp/pdfs"))


def run(*args):
    process = subprocess.run([str(arg) for arg in args], capture_output=True, text=True)
    if process.returncode:
        raise RuntimeError(f"{args}: exit {process.returncode}\n{process.stdout}\n{process.stderr}")
    return process.stdout + process.stderr


manifest = json.loads((FIX / "manifest.json").read_text())
for name, digest in manifest["sha256"].items():
    assert hashlib.sha256((FIX / name).read_bytes()).hexdigest() == digest, name
assert hashlib.sha256((ROOT / "font-data/fonts/n022003l.pfb").read_bytes()).hexdigest() == manifest["pfb_source_sha256"]

# Independent font-program parsing: each CFF wrapper has the same outlines and
# widths; TrueType has equivalent geometric paths (quadratics not required).
otf = TTFont(FIX / "geometry.otf", checkChecksums=2)
ttf = TTFont(FIX / "geometry.ttf", checkChecksums=2)
cff = CFFFontSet()
cff.decompile(io.BytesIO((FIX / "geometry.cff").read_bytes()), None)
for name, points, width in [("A", [(0, 0), (500, 0), (500, 700), (0, 700)], 600), ("B", [(0, 0), (600, 0), (300, 700)], 700)]:
    expected = [("moveTo", (points[0],))] + [("lineTo", (point,)) for point in points[1:]] + [("closePath", ())]
    for glyph in [cff.topDictIndex[0].CharStrings[name], otf.getGlyphSet()[name], ttf.getGlyphSet()[name]]:
        pen = RecordingPen()
        glyph.draw(pen)
        assert pen.value == expected, (name, pen.value)
    assert cff.topDictIndex[0].CharStrings[name].width == width
    assert ttf["hmtx"][name][0] == width

log = run(ROOT / "target/debug/examples/v07_font_roundtrip", FIX / "fonts.pdf", OUT / "actual.pdf")
(OUT / "driver.log").write_text(log)
maps = [line.split("\t") for line in log.splitlines() if line.startswith("MAP\t")]
chars = [line.split("\t") for line in log.splitlines() if line.startswith("CHAR\t")]
glyphs = [line.split("\t") for line in log.splitlines() if line.startswith("GLYPH\t")]
pfb = fitz.Font(fontbuffer=(ROOT / "font-data/fonts/n022003l.pfb").read_bytes())
for page, case in enumerate(manifest["cases"]):
    rows = [row for row in maps if int(row[1]) == page]
    assert len(rows) == len(case["codes"])
    for index, row in enumerate(rows):
        assert row[3] == case["codes"][index]
        expected_gid = case["gids"][index] if case["gids"] is not None else pfb.has_glyph(ord(("BA" if case["kind"] == "differences" else "AB")[index]))
        assert int(row[4]) == expected_gid, (case["kind"], row, expected_gid)
        assert float(row[5]) == case["widths"][index]
        assert [float(row[6]), float(row[7])] == case["origins"][index]
        assert bytes.fromhex(row[10]).decode() == case["unicode"][index]
    after = [row for row in chars if int(row[1]) == page]
    after_glyphs = [row for row in glyphs if int(row[1]) == page]
    assert len(after) == 2 * len(rows)
    for index, row in enumerate(after[len(rows):]):
        x, y = 30 + index * 30, 40 + (index % 2) * 15
        expected = [y, x] if case["rotation"] == 90 else [x, 180 - y]
        assert all(math.isclose(float(actual), target, abs_tol=.0001) for actual, target in zip(row[3:5], expected)), (case["kind"], row, expected)
        ox, oy = case["origins"][index]
        # Vertical origin displacement is subtracted before the page transform.
        x, y = x - ox * .02, y - oy * .02
        expected = [y, x] if case["rotation"] == 90 else [x, 180 - y]
        assert all(math.isclose(float(actual), target, abs_tol=.0001) for actual, target in zip(after_glyphs[len(rows)+index][3:5], expected)), (case["kind"], row, expected)
        assert bytes.fromhex(row[5]).decode() == case["unicode"][index]

# Independently append equivalent PDF text operators, without using rzpdf's
# writer or serialiser. Original font programs must survive byte-for-byte.
with pikepdf.open(FIX / "fonts.pdf") as reference:
    for page, case in zip(reference.pages, manifest["cases"]):
        body = page.Contents.read_bytes() + b"\nq\nBT /F0 20 Tf 0 Tc 0 Tw 100 Tz 0 Ts 0 Tr\n"
        for index, code in enumerate(case["codes"]):
            body += f"1 0 0 1 {30 + 30*index} {40 + 15*(index%2)} Tm <{code}> Tj\n".encode()
        page.Contents = reference.make_stream(body + b"ET\nQ\n")
    reference.save(OUT / "reference.pdf", deterministic_id=True)
with pikepdf.open(FIX / "fonts.pdf") as source, pikepdf.open(OUT / "actual.pdf") as actual:
    for original, saved in zip(source.pages, actual.pages):
        def program(page):
            font = page.Resources.Font.F0
            if font.Subtype == pikepdf.Name.Type0: font = font.DescendantFonts[0]
            if "/FontDescriptor" not in font: return None
            d = font.FontDescriptor
            return next(d[key].read_bytes() for key in ("/FontFile", "/FontFile2", "/FontFile3") if key in d)
        assert program(original) == program(saved)

checks = {}
for name in ("actual", "reference"):
    checks[name] = run("qpdf", "--check", OUT / f"{name}.pdf")
    run(ROOT / "target/debug/examples/render_document", OUT / f"{name}.pdf", OUT / f"{name}-skia")
    run("mutool", "draw", "-q", "-r", "72", "-o", OUT / f"{name}-mupdf-%d.png", OUT / f"{name}.pdf")
    run("mutool", "draw", "-q", "-F", "stext", "-o", OUT / f"{name}.xml", OUT / f"{name}.pdf")

pixels = []
for page in range(1, len(manifest["cases"]) + 1):
    for renderer in ("skia", "mupdf"):
        def path(name):
            return OUT / f"{name}-skia/skia-{page}.png" if renderer == "skia" else OUT / f"{name}-mupdf-{page}.png"
        actual, reference = (Image.open(path(name)).convert("RGB") for name in ("actual", "reference"))
        assert actual.size == reference.size
        assert ImageChops.difference(actual, reference).getbbox() is None, (page, renderer)
        assert ImageChops.difference(actual, Image.new("RGB", actual.size, "white")).getbbox(), (page, renderer, "blank page")
    # Different rasterizers may anti-alias differently. Both must put ink in
    # equivalent bounds; a fixed mean-channel tolerance is not pixel identity.
    skia = Image.open(OUT / f"actual-skia/skia-{page}.png").convert("RGB")
    mupdf = Image.open(OUT / f"actual-mupdf-{page}.png").convert("RGB")
    difference = ImageChops.difference(skia, mupdf)
    mean = sum(ImageStat.Stat(difference).mean) / 3
    assert mean <= 2.0, (page, mean)
    pixels.append({"page": page, "kind": manifest["cases"][page-1]["kind"], "cross_renderer_mean_channel_difference": mean})

# Independent producer extraction must agree for the saved vs reference file,
# including multi-scalar strings. Exact pixel identity above checks GID outlines.
with fitz.open(OUT / "actual.pdf") as a, fitz.open(OUT / "reference.pdf") as b:
    for page, case in enumerate(manifest["cases"]):
        assert a[page].get_text("rawdict") == b[page].get_text("rawdict"), case["kind"]
        text = a[page].get_text().replace("\n", "").replace(" ", "")
        assert text == "".join(case["unicode"]) * 2, (case["kind"], text)
        trace = [ch for span in a[page].get_texttrace() for ch in span["chars"] if ch[1] >= 0]
        gids = case["gids"] if case["gids"] is not None else [pfb.has_glyph(ord(c)) for c in ("BA" if case["kind"] == "differences" else "AB")]
        assert [ch[1] for ch in trace] == gids * 2, (case["kind"], trace)
        # MuPDF trace uses the horizontal outline origin. It is a separate
        # independent check of W2, not merely a comparison of two library runs.
        for index, ch in enumerate(trace[len(gids):]):
            ox, oy = case["origins"][index]
            expected = [30+30*index-ox*.02, 180-(40+15*(index%2)-oy*.02)]
            # Texttrace coordinates are unrotated even when page.Rotate=90.
            assert all(math.isclose(actual, target, abs_tol=.0001) for actual, target in zip(ch[2], expected)), (case["kind"], ch, expected)

report = {"head": run("git", "-C", ROOT, "rev-parse", "HEAD").strip(), "status": run("git", "-C", ROOT, "status", "--short").strip(), "tools": {"pymupdf": fitz.VersionBind, "pikepdf": pikepdf.__version__, "mutool": run("mutool", "-v").strip()}, "font_programs": "FontTools independent paths/widths and unchanged save bytes", "qpdf": checks, "pixels": pixels, "maps": len(maps), "saved_characters": len(chars)}
(OUT / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print(OUT / "report.json")
