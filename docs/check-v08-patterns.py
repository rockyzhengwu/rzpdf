"""Pattern interoperability gate. Build v08_patterns/render_document first.

Requires Pillow, pikepdf, qpdf and mutool. No Poppler, network or fuzz.
Every run creates a fresh evidence directory, including failed renders.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(tempfile.mkdtemp(prefix="v08-patterns-", dir=ROOT / "tmp/pdfs"))
print(OUT, flush=True)


def run(label, *args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    (OUT / f"{label}.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (label, result.returncode, result.stderr)
    return result.stdout


run("generate", ROOT / "target/debug/examples/v08_patterns", OUT / "patterns.pdf")
run("qpdf", "qpdf", "--check", OUT / "patterns.pdf")
with pikepdf.open(OUT / "patterns.pdf") as pdf:
    assert len(pdf.pages) == 12
    for page in pdf.pages[:7]:
        assert "/Pattern" in page.Resources
    assert "/Shading" in pdf.pages[7].Resources
    # PDF Reference table 4.24 says direct sh ignores Background. Installed
    # MuPDF 1.27 instead paints it. Preserve its original raster, and render
    # the standard-equivalent resource without that ignored entry as oracle.
    direct = next(value for _, value in pdf.pages[7].Resources.Shading.items())
    assert list(direct.Background) == [0,1,0]
    del direct["/Background"]
    pdf.save(OUT / "direct-sh-reference.pdf")
run("skia", ROOT / "target/debug/examples/render_document", OUT / "patterns.pdf", OUT / "skia")
run("mupdf", "mutool", "draw", "-q", "-r", "72", "-o", OUT / "mupdf-%d.png", OUT / "patterns.pdf")
run("mupdf-shading-centers", "mutool", "draw", "-q", "-r", "576", "-o", OUT / "mupdf-hi-%d.png", OUT / "patterns.pdf", "7,8")
run("mupdf-direct-standard", "mutool", "draw", "-q", "-r", "576", "-o", OUT / "mupdf-direct-%d.png", OUT / "direct-sh-reference.pdf", "8")
run("skia-nonuniform", ROOT / "target/debug/examples/render_document", OUT / "patterns.pdf", OUT / "skia-nonuniform", "144", "96")
run("mupdf-nonuniform-area", "mutool", "draw", "-q", "-w", "360", "-h", "240", "-f", "-o", OUT / "mupdf-nonuniform-%d.png", OUT / "patterns.pdf", "1-6,9-12")

# Freeze this threshold before observing output. Pixel-center checks below
# independently cover lattice, gaps, and Pattern-only Background semantics.
MEAN_CHANNEL_LIMIT = 2.0
rows = []
for page in range(1, 13):
    actual = Image.open(OUT / f"skia/skia-{page}.png").convert("RGB")
    expected = Image.open(OUT / f"mupdf-{page}.png").convert("RGB")
    native_mean = sum(ImageStat.Stat(ImageChops.difference(actual, expected)).mean) / 3
    if page in (7, 8):
        # MuPDF tessellates shading bounds at device resolution. At 72 dpi
        # its endpoint can move by a whole pixel. Sample 576 dpi output at
        # the 72 dpi pixel centers instead; keep the native output and metric.
        reference = f"mupdf-hi-{page}.png" if page == 7 else "mupdf-direct-8.png"
        expected = Image.open(OUT / reference).convert("RGB").resize((60,60), Image.Resampling.NEAREST)
    assert actual.size == expected.size == (60, 60)
    difference = ImageChops.difference(actual, expected)
    difference.save(OUT / f"difference-{page}.png")
    mean = sum(ImageStat.Stat(difference).mean) / 3
    rows.append({"page": page, "mean_channel_difference": mean, "native_72dpi_difference": native_mean,
                 "reference_sampling": "576dpi centers" if page in (7,8) else "72dpi"})
    print(rows[-1], flush=True)
    assert mean <= MEAN_CHANNEL_LIMIT, rows[-1]
    if page in (1, 2, 7, 8):
        points = {1: [(1, 58), (6, 58)], 2: [(3, 55), (8, 55)], 7: [(5, 30), (30, 30)], 8: [(5, 30), (30, 30)]}[page]
        for point in points:
            assert max(abs(a-b) for a,b in zip(actual.getpixel(point), expected.getpixel(point))) <= 2, (page, point)

report = {
    "source_head": run("head", "git", "-C", ROOT, "rev-parse", "HEAD").strip(),
    "working_tree": run("status", "git", "-C", ROOT, "status", "--short"),
    "pdf_sha256": hashlib.sha256((OUT / "patterns.pdf").read_bytes()).hexdigest(),
    "mean_channel_limit": MEAN_CHANNEL_LIMIT,
    "reader_differences": ["MuPDF 72dpi shading endpoint tessellation; compare 576dpi centers without raising colour tolerance",
                           "MuPDF applies Background to direct sh; project ignores it per PDF Reference table 4.24; independent reference removes only that ignored entry"],
    "pages": rows,
}
nonuniform = []
for page in [1,2,3,4,5,6,9,10,11,12]:
    actual = Image.open(OUT / f"skia-nonuniform/skia-{page}.png").convert("RGB")
    expected = Image.open(OUT / f"mupdf-nonuniform-{page}.png").convert("RGB")
    assert actual.size == (120,80) and expected.size == (360,240)
    expected = expected.resize(actual.size,Image.Resampling.BOX)
    mean = sum(ImageStat.Stat(ImageChops.difference(actual,expected)).mean)/3
    row = {"page":page,"mean_channel_difference":mean,"reference_sampling":"3x3 premultiplied area"}
    assert mean <= MEAN_CHANNEL_LIMIT, row
    if page == 6:
        assert all(pixel[1] == 0 for pixel in actual.getdata()), "white seams in fully covered overlapping cells"
    nonuniform.append(row)
report["nonuniform_144x96_dpi"] = nonuniform
(OUT / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print("PASS", OUT, flush=True)
