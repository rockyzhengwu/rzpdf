"""Independent Type3 model checkpoint, not a Type3 production-render gate.

Build pdf/example inspect_type3_font first. Uses pikepdf and MuPDF, no Poppler.
All pictures/programs are project-authored; evidence goes in a fresh directory.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

import pikepdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = Path(tempfile.mkdtemp(prefix="v07-type3-model-", dir=ROOT / "tmp/pdfs"))


def run(*args):
    result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip() or result.stderr.strip()


with pikepdf.Pdf.new() as pdf:
    page = pdf.add_blank_page(page_size=(100, 100))
    font = pdf.make_indirect(pikepdf.Dictionary(
        Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type3,
        FontBBox=pikepdf.Array([0, 0, 600, 700]),
        FontMatrix=pikepdf.Array([.001, 0, 0, .001, 0, 0]),
        FirstChar=65, LastChar=66, Widths=pikepdf.Array([600, 700]),
        Encoding=pikepdf.Dictionary(Differences=pikepdf.Array([65, pikepdf.Name.A, pikepdf.Name.B])),
        Resources=pikepdf.Dictionary(ProcSet=pikepdf.Array([pikepdf.Name.PDF])),
        CharProcs=pikepdf.Dictionary(
            A=pdf.make_stream(b"600 0 d0 1 0 0 rg 0 0 500 700 re f"),
            B=pdf.make_stream(b"700 0 0 0 600 700 d1 0 0 m 600 0 l 300 700 l h f"),
        ),
    ))
    page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F0=font))
    page.Contents = pdf.make_stream(b"BT /F0 36 Tf 1 0 0 1 20 30 Tm (AB) Tj ET")
    pdf.save(OUTPUT / "source.pdf", min_version="1.7")

reports = []
for code in (65, 66):
    args = [ROOT / "target/debug/examples/inspect_type3_font", OUTPUT / "source.pdf", 0, "F0", code]
    if code == 65:
        args.append(OUTPUT / "roundtrip.pdf")
    report = json.loads(run(*args))
    assert report["font_matrix"] == [.001, 0, 0, .001, 0, 0]
    assert report["font_bbox"] == [0, 0, 600, 700]
    width = 600 if code == 65 else 700
    assert report["width"] == width
    assert max(abs(a - b) for a, b in zip(report["advance"], [width / 1000, 0])) < 1e-6
    assert report["glyph_width"] == [width, 0]
    assert report["glyph_bbox"] == (None if code == 65 else [0, 0, 600, 700])
    assert report["operations"] == (4 if code == 65 else 6)
    reports.append(report)

pixels = []
for name in ("source", "roundtrip"):
    path = OUTPUT / f"{name}.pdf"
    run("qpdf", "--check", path)
    with pikepdf.Pdf.open(path, attempt_recovery=False) as pdf:
        assert not pdf.check_pdf_syntax()
        assert list(pdf.pages[0].Resources.Font.F0.Widths) == [600, 700]
        assert pdf.pages[0].Resources.Font.F0.CharProcs.A.read_bytes() == b"600 0 d0 1 0 0 rg 0 0 500 700 re f"
    run("mutool", "draw", "-q", "-r", 72, "-o", OUTPUT / f"{name}.png", path)
    image = Image.open(OUTPUT / f"{name}.png").convert("RGB")
    assert image.size == (100, 100)
    assert image.getpixel((25, 50)) == (255, 0, 0)
    assert image.getpixel((52, 60)) == (0, 0, 0)
    pixels.append(image.tobytes())
assert pixels[0] == pixels[1]
result = {
    "scope": "Type3 model inspection and preservation only; no rzpdf Type3 execution",
    "source_commit": run("git", "-C", ROOT, "rev-parse", "HEAD"),
    "working_tree": run("git", "-C", ROOT, "status", "--porcelain"),
    "pikepdf": pikepdf.__version__, "mupdf": run("mutool", "-v"),
    "glyphs": reports, "pixels_sha256": hashlib.sha256(pixels[0]).hexdigest(),
    "evidence": str(OUTPUT),
}
(OUTPUT / "report.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
