"""Independent bounded Type3 graphics execution gate (pikepdf/MuPDF/Skia).

Project-authored Type3 programs and independently expanded graphics share
explicit page transforms. Not a gate for arbitrary nested-text glyph programs.
Build render_document first. Creates fresh evidence, never replaces old files.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

import pikepdf
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(tempfile.mkdtemp(prefix="v07-type3-render-", dir=ROOT / "tmp/pdfs"))


def run(*args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip() or result.stderr.strip()


def mul(left, right):
    a, b, c, d, e, f = left
    g, h, i, j, k, l = right
    return [a*g+b*i, a*h+b*j, c*g+d*i, c*h+d*j, e*g+f*i+k, e*h+f*j+l]


def numbers(values):
    return " ".join(f"{value:.9g}" for value in values)


CASES = [
    {"name": "normal"},
    {"name": "rotate90", "rotate": 90},
    {"name": "rotate180", "rotate": 180},
    {"name": "rotate270", "rotate": 270},
    {"name": "horizontal-rise", "horizontal": .5, "rise": 4},
    {"name": "matrix", "matrix": [.002, .0005, .0003, .0015, .02, -.01]},
    {"name": "negative-size", "size": -24, "origin": [70, 65]},
    {"name": "negative-horizontal", "horizontal": -.75, "origin": [70, 30]},
    {"name": "clip", "clip": True},
    {"name": "image", "kind": "image"},
    {"name": "stencil", "kind": "stencil"},
    {"name": "form", "kind": "form"},
]

for expanded in (False, True):
    with pikepdf.Pdf.new() as pdf:
        for case in CASES:
            page = pdf.add_blank_page(page_size=(100, 100))
            page.Rotate = case.get("rotate", 0)
            size = case.get("size", 24)
            horizontal = case.get("horizontal", 1)
            rise = case.get("rise", 0)
            origin = case.get("origin", [20, 30])
            matrix = case.get("matrix", [.001, 0, 0, .001, 0, 0])
            # B is an uncoloured stroked glyph. It inherits the page stroke
            # colour, distinct from its inherited fill and A's explicit colour.
            a = (b"q 500 0 0 700 0 0 cm BI /W 1 /H 1 /CS /RGB /BPC 8 /F /AHx ID FF0000> EI Q"
                 if case.get("kind") == "image" else b"1 0 0 rg 0 0 500 700 re f")
            # Raster expectations remain raster draws: comparing a nearest-
            # sampled mask with an antialiased vector edge is not equivalent.
            b = (b"q 600 0 0 700 0 0 cm BI /W 1 /H 1 /IM true /F /AHx ID 00> EI Q"
                 if case.get("kind") == "stencil" else b"0 0 600 700 re S")
            prefix = "100 w 0 0 1 rg 0 1 0 RG "
            if case.get("clip"):
                prefix += "22 32 20 10 re W n "
            if not expanded:
                glyph_a, glyph_b = a, b
                xobjects = pikepdf.Dictionary()
                if case.get("kind") == "image":
                    image = pdf.make_stream(bytes([255, 0, 0]))
                    image.Type = pikepdf.Name.XObject
                    image.Subtype = pikepdf.Name.Image
                    image.Width, image.Height = 1, 1
                    image.BitsPerComponent = 8
                    image.ColorSpace = pikepdf.Name.DeviceRGB
                    xobjects.Image = image
                    glyph_a = b"q 500 0 0 700 0 0 cm /Image Do Q"
                if case.get("kind") == "stencil":
                    mask = pdf.make_stream(bytes([0]))
                    mask.Type = pikepdf.Name.XObject
                    mask.Subtype = pikepdf.Name.Image
                    mask.Width, mask.Height = 1, 1
                    mask.ImageMask = True
                    xobjects.Mask = mask
                    glyph_b = b"q 600 0 0 700 0 0 cm /Mask Do Q"
                if case.get("kind") == "form":
                    form = pdf.make_stream(a)
                    form.Type = pikepdf.Name.XObject
                    form.Subtype = pikepdf.Name.Form
                    form.BBox = pikepdf.Array([0, 0, 500, 700])
                    form.Resources = pikepdf.Dictionary()
                    xobjects.Inner = form
                    glyph_a = b"/Inner Do"
                font = pdf.make_indirect(pikepdf.Dictionary(
                    Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type3,
                    FontMatrix=pikepdf.Array(matrix), FontBBox=pikepdf.Array([-50, -50, 650, 750]),
                    FirstChar=65, LastChar=66, Widths=pikepdf.Array([600, 700]),
                    Encoding=pikepdf.Dictionary(Differences=pikepdf.Array([65, pikepdf.Name.A, pikepdf.Name.B])),
                    CharProcs=pikepdf.Dictionary(
                        A=pdf.make_stream(b"600 0 d0 " + glyph_a),
                        B=pdf.make_stream(b"700 0 -50 -50 650 750 d1 " + glyph_b),
                    ),
                ))
                if xobjects:
                    font.Resources = pikepdf.Dictionary(XObject=xobjects)
                page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F0=font))
                body = prefix + f"BT /F0 {size} Tf {horizontal*100} Tz {rise} Ts {case.get('mode', 0)} Tr 1 0 0 1 {numbers(origin)} Tm (AB) Tj ET"
            else:
                parts = [prefix]
                x, y = origin
                for width, program in [(600, a), (700, b)]:
                    transform = mul(mul(matrix, [size*horizontal, 0, 0, size, 0, 0]), [1, 0, 0, 1, x, y+rise])
                    parts.append("q " + numbers(transform) + " cm ")
                    parts.append(program.decode() + " Q ")
                    x += width * matrix[0] * size * horizontal
                    # Simple-font text displacement stays horizontal even
                    # when FontMatrix rotates/shears the glyph description.
                body = "".join(parts)
            page.Contents = pdf.make_stream(body.encode())
        pdf.save(OUT / ("expanded.pdf" if expanded else "type3.pdf"), min_version="1.7")

for name in ("type3", "expanded"):
    path = OUT / f"{name}.pdf"
    run("qpdf", "--check", path)
    run(ROOT / "target/debug/examples/render_document", path, OUT / name)
    run("mutool", "draw", "-q", "-r", 72, "-o", OUT / f"mu-{name}-%d.png", path)

run(ROOT / "target/debug/examples/inspect_type3_font", OUT / "type3.pdf", 0, "F0", 65, OUT / "preserved.pdf")
run("qpdf", "--check", OUT / "preserved.pdf")
run(ROOT / "target/debug/examples/render_document", OUT / "preserved.pdf", OUT / "preserved")
run("mutool", "draw", "-q", "-r", 72, "-o", OUT / "mu-preserved-%d.png", OUT / "preserved.pdf")
with pikepdf.Pdf.open(OUT / "type3.pdf") as original, pikepdf.Pdf.open(OUT / "preserved.pdf") as saved:
    assert len(original.pages) == len(saved.pages) == len(CASES)
    for before, after in zip(original.pages, saved.pages):
        for key in ("A", "B"):
            assert before.Resources.Font.F0.CharProcs["/" + key].read_raw_bytes() == after.Resources.Font.F0.CharProcs["/" + key].read_raw_bytes()

rows = []
for index, case in enumerate(CASES, 1):
    images = [Image.open(OUT / name / f"skia-{index}.png").convert("RGB") for name in ("type3", "expanded")]
    assert Image.open(OUT / "preserved" / f"skia-{index}.png").convert("RGB").tobytes() == images[0].tobytes()
    # Independent expansion and actual Type3 must agree up to rounding at
    # vector edges. Predeclared mean-channel tolerance: 0.1 of 255 per page.
    delta = ImageChops.difference(*images)
    mean = sum(delta.tobytes()) / len(delta.tobytes())
    assert mean <= .1, (case, "Skia mean sample error", mean)
    white = Image.new("RGB", (100, 100), "white")
    bounds = [ImageChops.difference(image, white).getbbox() for image in images]
    assert bounds[0] == bounds[1], (case, bounds)
    mu = [Image.open(OUT / f"mu-{name}-{index}.png").convert("RGB") for name in ("type3", "expanded")]
    assert Image.open(OUT / f"mu-preserved-{index}.png").convert("RGB").tobytes() == mu[0].tobytes()
    mu_delta = ImageChops.difference(*mu)
    mu_mean = sum(mu_delta.tobytes()) / len(mu_delta.tobytes())
    # MuPDF may cache and sample a Type3 mask differently from direct graphics.
    assert mu_mean <= 1.0, (case, "MuPDF mean sample error", mu_mean)
    mu_bounds = [ImageChops.difference(image, white).getbbox() for image in mu]
    assert all(abs(a-b) <= 1 for a, b in zip(mu_bounds[0], mu_bounds[1])), (case, mu_bounds)
    assert all(abs(a-b) <= 1 for a, b in zip(bounds[0], mu_bounds[0])), (case, bounds, mu_bounds)
    # Direct-graphics comparison also ties both renderers together; it must not
    # be possible for both Type3 paths to agree on an entirely wrong colour.
    cross = ImageChops.difference(images[1], mu[1])
    assert sum(cross.tobytes()) / len(cross.tobytes()) <= 1.0, (case, "cross-renderer graphics")
    rows.append({"case":case["name"],"skia_mean_error":mean,"mupdf_mean_error":mu_mean,"bounds":bounds[0],"sha256":hashlib.sha256(images[0].tobytes()).hexdigest()})

negative = []
for mode in (3, 7):
    with pikepdf.Pdf.open(OUT / "type3.pdf") as pdf:
        source = pdf.pages[0].Contents.read_bytes().replace(b"0 Tr", f"{mode} Tr".encode())
        pdf.pages[0].Contents = pdf.make_stream(source)
        path = OUT / f"unsupported-Tr{mode}.pdf"
        pdf.save(path)
    result = subprocess.run([str(ROOT/"target/debug/examples/render_document"),str(path),str(OUT/f"unsupported-Tr{mode}")],capture_output=True,text=True)
    assert result.returncode != 0 and "Type3 non-default text rendering mode outside baseline" in result.stderr, result
    negative.append({"Tr":mode,"result":"explicit UnsupportedFeature; not a passing pixel case"})

report = {"source_commit":run("git","-C",ROOT,"rev-parse","HEAD"),"working_tree":run("git","-C",ROOT,"status","--porcelain"),"scope":"opaque graphics-only Type3, Tr=0; not arbitrary glyph text or transparency", "mupdf":run("mutool","-v"),"pikepdf":pikepdf.__version__,"cases":rows,"negative":negative,"evidence":str(OUT)}
(OUT / "report.json").write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps(report,indent=2))
