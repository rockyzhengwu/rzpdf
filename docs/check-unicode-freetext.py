"""Independent FreeText appearance gate: qpdf, MuPDF and PDFium.

python3 docs/check-unicode-freetext.py CORPUS_DIRECTORY NEW_OUTPUT_DIRECTORY
The corpus contains native created/updated.pdf, or browser sync/worker PDFs.
Requires existing PyMuPDF, pypdfium2 and Pillow; no installs or downloads.
"""
import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys

import fitz
import pypdfium2 as pdfium
from PIL import Image, ImageChops, ImageStat

CORPUS = Path(sys.argv[1]).resolve()
OUT = Path(sys.argv[2]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
files = [p for p in sorted(CORPUS.glob("*.pdf")) if p.stem in ("created", "updated") or p.stem.startswith(("sync-", "worker-"))]
assert files, "no authored output corpus"
records = []


def ink(image):
    pixels = getattr(image, "get_flattened_data", image.getdata)()
    return sum(1 for pixel in pixels if min(pixel[:3]) < 235)


def rectangle(document, xref, key):
    kind, value = document.xref_get_key(xref, key)
    assert kind == "array", (xref, key, kind)
    values = [float(n) for n in re.findall(r"[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)", value)]
    assert len(values) == 4
    return values


def unrotated_x(page, bbox):
    # Use the actual viewport dimensions. PyMuPDF 1.26's derotation translation
    # does not scale with UserUnit, so its raw derotation matrix is unsuitable
    # for this combined rotation/UserUnit fixture.
    points = [(bbox[x], bbox[y]) for x in (0, 2) for y in (1, 3)]
    if page.rotation == 90:
        values = [y for x, y in points]
    elif page.rotation == 180:
        values = [page.rect.width - x for x, y in points]
    elif page.rotation == 270:
        values = [page.rect.height - y for x, y in points]
    else:
        values = [x for x, y in points]
    return min(values), max(values)


for path in files:
    encrypted = path.stem.endswith("encrypted")
    password = "owner" if encrypted else None
    check = subprocess.run(["qpdf"] + (["--password=" + password] if password else []) + ["--check", str(path)], capture_output=True, text=True, timeout=60)
    (OUT / (path.stem + "-qpdf.log")).write_text(check.stdout + check.stderr)
    assert check.returncode == 0, (path.name, check.stderr)
    m = fitz.open(path)
    if encrypted:
        assert m.needs_pass and m.authenticate(password)
    p = pdfium.PdfDocument(path, password=password)
    expected_count = 5 if path.stem.endswith("updated") and path.stem.startswith(("sync-", "worker-")) else 4
    assert len(m) == len(p) == expected_count
    appearances = []
    comparisons = []
    for index in range(len(m)):
        mp = m[index]
        kind, value = m.xref_get_key(mp.xref, "UserUnit")
        unit = float(value) if kind in ("int", "float") else 1.0
        annotations = list(mp.annots() or [])
        expected_annotations = (1 if index == 0 else 0) if encrypted else 3
        assert len(annotations) == expected_annotations, (path.name, index)
        for a in annotations:
            assert a.type[1] == "FreeText"
            contents = a.info["content"]
            extracted = a.get_text().rstrip("\n")
            assert extracted == contents.rstrip("\n"), (path.name, index, extracted, contents)
            assert "\ufffd" not in extracted and extracted
            ap = a.get_pixmap(alpha=True)
            assert any(ap.samples[i] > 0 for i in range(3, len(ap.samples), 4)), "blank appearance"
            q = int(m.xref_get_key(a.xref, "Q")[1])
            pdf_rect = rectangle(m, a.xref, "Rect")
            crop = rectangle(m, mp.xref, "CropBox")
            rect_left = (pdf_rect[0] - crop[0]) * unit
            rect_right = (pdf_rect[2] - crop[0]) * unit
            for block in a.get_text("dict")["blocks"]:
                for line in block["lines"]:
                    spans = line["spans"]
                    left = min(unrotated_x(mp, s["bbox"])[0] for s in spans)
                    right = max(unrotated_x(mp, s["bbox"])[1] for s in spans)
                    # Text extraction boxes include font metrics; permit a small
                    # side-bearing allowance, scaled with the declared UserUnit.
                    if q == 0:
                        assert abs(left - (rect_left + 2 * unit)) <= 2 * unit
                    elif q == 1:
                        assert abs((left + right) / 2 - (rect_left + rect_right) / 2) <= 2 * unit
                    else:
                        assert abs(right - (rect_right - 2 * unit)) <= 2 * unit
            appearances.append(dict(page=index, text=extracted, alignment=q, font_size=a.get_text("dict")["blocks"][0]["lines"][0]["spans"][0]["size"]))
        pp = p[index]
        for dpi in (72, 144):
            pixmap = mp.get_pixmap(dpi=dpi, annots=True, alpha=False)
            mu = Image.frombytes("RGB", (pixmap.width, pixmap.height), pixmap.samples)
            # PDFium's page-size/render helpers expose default user-space units.
            # Apply the PDF's actual UserUnit explicitly for matching output DPI.
            bitmap = pp.render(scale=dpi / 72 * unit, draw_annots=True, may_draw_forms=False)
            other = bitmap.to_pil().convert("RGB").copy()
            bitmap.close()
            assert mu.size == other.size, (path.name, index, dpi, mu.size, other.size)
            mu_ink, pdfium_ink = ink(mu), ink(other)
            if annotations:
                assert mu_ink > 20 and pdfium_ink > 20
                assert 0.65 <= pdfium_ink / mu_ink <= 1.55
            diff = ImageChops.difference(mu, other)
            mae = sum(ImageStat.Stat(diff).mean) / 3 / 255
            assert mae <= 0.025, (path.name, index, dpi, mae)
            # Keep standalone images for visual verification/export.
            mu.save(OUT / f"{path.stem}-p{index}-{dpi}-mupdf.png")
            other.save(OUT / f"{path.stem}-p{index}-{dpi}-pdfium.png")
            comparisons.append(dict(page=index,dpi=dpi,width=mu.width,height=mu.height,mupdf_ink=mu_ink,pdfium_ink=pdfium_ink,mae=mae))
        pp.close()
    assert appearances
    records.append(dict(file=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),appearances=appearances,comparisons=comparisons))
    m.close()
    p.close()

(OUT / "report.json").write_text(json.dumps(dict(status="passed",pymupdf=fitz.VersionBind,pdfium=str(pdfium.PDFIUM_INFO),pypdfium2=str(pdfium.PYPDFIUM_INFO),records=records),ensure_ascii=False,indent=2) + "\n")
print(f"PASS: {len(records)} PDFs; {sum(len(r['appearances']) for r in records)} Unicode AP text/alignment checks; {sum(len(r['comparisons']) for r in records)} MuPDF/PDFium page pairs")
