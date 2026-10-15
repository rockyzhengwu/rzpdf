"""Independent Type7 gate: stream controls, analytic inverse, and MuPDF.

Build v08_tensor/render_document; run with Python 3.11 + Pillow + pikepdf.
No network, Poppler, or fuzz. Failed evidence is retained.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(tempfile.mkdtemp(prefix="v08-tensor-", dir=ROOT / "tmp/pdfs"))
print(OUT, flush=True)


def run(label, *args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    (OUT / f"{label}.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (label, result.returncode, result.stderr)
    return result.stdout


run("generate", ROOT / "target/debug/examples/v08_tensor", OUT / "tensor.pdf")
run("qpdf", "qpdf", "--check", OUT / "tensor.pdf")
with pikepdf.open(OUT / "tensor.pdf") as pdf:
    assert len(pdf.pages) == 4
    for page_index, page in enumerate(pdf.pages):
        shade = next(value for _, value in page.Resources.Shading.items())
        assert int(shade.ShadingType) == 7
        data = shade.read_bytes()
        assert len(data) == 37 and data[0] == 0 and data[33:] == bytes([0,0,255,255])
        assert data[25:27] == bytes([144,80] if page_index >= 2 else [80,80])
        assert ("/Function" in shade) == (page_index % 2 == 1)
    # Independently produce Type6 records from the affine Type7 boundary.
    # Deleting its interior controls is valid only for these affine pages.
    for page in pdf.pages[:2]:
        shade=next(value for _,value in page.Resources.Shading.items())
        data=shade.read_bytes();shade.write(data[:25]+data[33:]);shade.ShadingType=6
    del pdf.pages[2:]
    pdf.save(OUT/"coons.pdf")

run("skia", ROOT / "target/debug/examples/render_document", OUT / "tensor.pdf", OUT / "skia")
run("mupdf", "mutool", "draw", "-q", "-r", "576", "-o", OUT / "mupdf-%d.png", OUT / "tensor.pdf")
rows = []
for page in range(1,5):
    actual = Image.open(OUT / f"skia/skia-{page}.png").convert("RGB")
    expected = Image.open(OUT / f"mupdf-{page}.png").convert("RGB").resize((40,40),Image.Resampling.NEAREST)
    assert actual.size == (40,40)
    moved = page >= 3
    squared = page % 2 == 0
    # Only the first interior x control is displaced: x=240u+64 B1(u)B1(v),
    # y=240v. This x function is strictly increasing, so bisection is an
    # independent inverse, not another implementation of the interval solver.
    maximum = 0
    for y in range(40):
        for x in range(40):
            px = ((x+.5)-2)*10
            py = (38-(y+.5))*10
            value = 255
            if 0 <= px <= 240 and 0 <= py <= 240:
                v = py/240
                low, high = 0.,1.
                for _ in range(60):
                    u = (low+high)*.5
                    mapped = 240*u + (64*3*u*(1-u)**2 * 3*v*(1-v)**2 if moved else 0)
                    if mapped < px: low = u
                    else: high = u
                u = (low+high)*.5
                value = int(255*(u*u if squared else u)+.5)
            error = max(abs(channel-value) for channel in actual.getpixel((x,y)))
            maximum = max(maximum,error)
    assert maximum <= 1, (page, maximum)
    difference = ImageChops.difference(actual,expected)
    difference.save(OUT / f"difference-{page}.png")
    mean = sum(ImageStat.Stat(difference).mean)/3
    row = {"page":page,"analytic_max_channel_error":maximum,"mupdf_mean_channel_difference":mean}
    print(row,flush=True)
    assert mean <= 2.0, row
    rows.append(row)

run("qpdf-coons","qpdf","--check",OUT/"coons.pdf")
run("skia-coons",ROOT/"target/debug/examples/render_document",OUT/"coons.pdf",OUT/"skia-coons")
run("mupdf-coons","mutool","draw","-q","-r","576","-o",OUT/"coons-%d.png",OUT/"coons.pdf")
coons=[]
for page in [1,2]:
    actual=Image.open(OUT/f"skia-coons/skia-{page}.png").convert("RGB")
    tensor=Image.open(OUT/f"skia/skia-{page}.png").convert("RGB")
    assert ImageChops.difference(actual,tensor).getbbox() is None
    reference=Image.open(OUT/f"coons-{page}.png").convert("RGB").resize((40,40),Image.Resampling.NEAREST)
    mean=sum(ImageStat.Stat(ImageChops.difference(actual,reference)).mean)/3
    assert mean<=2,(page,mean);coons.append(dict(page=page,analytic_max_channel_error=rows[page-1]["analytic_max_channel_error"],mupdf_mean_channel_difference=mean))

report = {"source_head":run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),
          "working_tree":run("status","git","-C",ROOT,"status","--short"),
          "pdf_sha256":hashlib.sha256((OUT/"tensor.pdf").read_bytes()).hexdigest(),"pages":rows,"type6":coons}
(OUT/"report.json").write_text(json.dumps(report,indent=2)+"\n")
print("PASS",OUT,flush=True)
