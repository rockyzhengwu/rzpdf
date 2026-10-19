"""Associated-file edit/save/page-operation gate. AGPL-3.0-only.
Build pdf --example v014_associated_files first, then use the existing Python environment.
All inputs/outputs are original synthetic fixtures; no input file is overwritten.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import os

import pikepdf
from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
BIN = ROOT / "target/debug/examples/v014_associated_files"


def run(label, *args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (label, result.stderr)
    return result.stdout + result.stderr


driver_hash = hashlib.sha256(BIN.read_bytes()).hexdigest()
SKIA = ROOT / "target/debug/examples/render_optional_content"
skia_hash = hashlib.sha256(SKIA.read_bytes()).hexdigest()
run("producers", sys.executable, ROOT / "docs/check-v014-producers.py", OUT / "inputs")
records, pictures = [], []
for producer in ["pikepdf", "mupdf"]:
    source = OUT / "inputs" / (producer + ".pdf")
    run(producer + "-input-skia", SKIA, source, OUT / (producer + "-input-skia"), "default", "view")
    text_reference = run(producer + "-input-text", "mutool", "draw", "-q", "-F", "txt", "-o", "-", source)
    result = run(producer + "-driver", BIN, source, OUT / (producer + "-outputs"))
    ids = re.search(r"cloned=(\d+) created=(\d+) tracked_peak=(\d+)", result)
    assert ids, result
    cloned, created, peak = map(int, ids.groups())
    for output in sorted((OUT / (producer + "-outputs")).glob("*.pdf")):
        label = producer + "-" + output.stem
        encrypted = "-r6" in output.stem
        appended = "-append-" in output.stem
        password = "user" if encrypted else ""
        run(label + "-qpdf", "qpdf", *(["--password=user"] if encrypted else []), "--check", output)
        relations = [["Source", "Data"], ["Source", "Supplement"]]
        catalog = ["Source", "Data"]
        if appended: relations[0] = ["Source"]
        if output.stem == "page-copy": relations.append(["Source", "Data"])
        if output.stem == "page-reorder": relations.reverse()
        if output.stem == "page-delete": relations = relations[:1]
        if output.stem == "page-extract": relations, catalog = relations[1:], []
        if output.stem == "page-import": catalog = []
        with pikepdf.open(output, password=password, attempt_recovery=False) as pdf:
            assert len(pdf.pages) == len(relations)
            assert [str(s.AFRelationship)[1:] for s in pdf.Root.get("/AF", [])] == catalog
            original = pdf.pages[0].AF[0]
            assert str(original.AFRelationship) == "/Source"
            for page, expected in zip(pdf.pages, relations):
                assert [str(s.AFRelationship)[1:] for s in page.AF] == expected
                assert page.AF[0].objgen == original.objgen
            if catalog:
                assert len(pdf.Root.OriginalAF) == 1
                assert pdf.Root.OriginalAF[0].objgen == original.objgen
            assert "/Names" not in pdf.Root
            for owner in [pdf.Root, *[p.obj for p in pdf.pages]]:
                for spec in owner.get("/AF", []):
                    relationship = str(spec.AFRelationship)
                    payload, mime = {"/Source": (b"AF fixture\n", "/text/plain"), "/Data": (b"AF fixture\n", "/text/plain"), "/Supplement": (b"new\0\xffdata", "/application/octet-stream")}[relationship]
                    stream = spec.EF.UF if "/UF" in spec.EF else spec.EF.F
                    assert stream.read_bytes() == payload and str(stream.Subtype) == mime
                    if relationship != "/Source":
                        assert spec.EF.F.objgen == spec.EF.UF.objgen
                        assert str(spec.UF) == ("复用源文件.txt" if relationship == "/Data" else "新附件.bin")
            if appended:
                base = OUT / (producer + "-outputs") / (output.stem.split("-append-")[0] + ".pdf")
                assert output.read_bytes().startswith(base.read_bytes())
                with pikepdf.open(base, password=password) as previous:
                    if "/ID" in previous.trailer:
                        assert bytes(previous.trailer.ID[0]) == bytes(pdf.trailer.ID[0])
                    else:
                        assert "/ID" not in pdf.trailer
                    if encrypted: assert previous.trailer.Encrypt.objgen == pdf.trailer.Encrypt.objgen
        clear = output
        if encrypted:
            clear = OUT / (label + "-decrypted.pdf")
            run(label + "-decrypt", "qpdf", "--password=user", "--decrypt", output, clear)
        run(label + "-render", "mutool", "draw", "-q", "-r", "72", "-o", OUT / (label + "-%d.png"), clear)
        run(label + "-skia", SKIA, clear, OUT / (label + "-skia"), "default", "view")
        text = run(label + "-text", "mutool", "draw", "-q", "-F", "txt", "-o", "-", clear)
        assert text.count("AF fixture text") == len(relations)
        for page in range(1, len(relations)+1):
            actual = OUT / (label + f"-{page}.png")
            a = Image.open(actual).convert("RGB")
            expected = Image.open(OUT / "inputs" / (producer + "-1.png")).convert("RGB")
            assert a.size == expected.size == (200, 200)
            assert ImageChops.difference(a, expected).getbbox() is None
            skia = Image.open(OUT / (label + "-skia") / f"page-{page}.png").convert("RGB")
            skia_reference = Image.open(OUT / (producer + "-input-skia") / "page-1.png").convert("RGB")
            assert skia.size == skia_reference.size == (200,200)
            assert ImageChops.difference(skia, skia_reference).getbbox() is None
            pictures.append((label + f" p{page}", actual))
        records.append(dict(case=label, sha256=hashlib.sha256(output.read_bytes()).hexdigest(), pixels=len(relations), tracked_peak=peak, encrypted=encrypted, appended=appended))

for start in range(0,len(pictures),24):
    sheet = Image.new("RGB", (1000, 6 * 235), "#ddd")
    draw = ImageDraw.Draw(sheet)
    for i, (label, path) in enumerate(pictures[start:start+24]):
        x, y = (i % 4) * 250, (i // 4) * 235
        sheet.paste(Image.open(path).convert("RGB"), (x + 25, y + 30))
        draw.text((x + 3, y + 3), label, fill="black")
    sheet.save(OUT / f"contact-{start//24+1}.png")
assert hashlib.sha256(BIN.read_bytes()).hexdigest() == driver_hash
assert hashlib.sha256(SKIA.read_bytes()).hexdigest() == skia_hash
manifest = dict(stage="AF authoring profile", license="AGPL-3.0-only", driver_sha256=driver_hash, skia_sha256=skia_hash,
                producer_manifest=json.loads((OUT / "inputs/manifest.json").read_text()), records=records,
                outputs=len(records), exact_pixel_pairs=len(pictures), skia_pixel_pairs=len(pictures))
(OUT / "manifest.json").write_text(json.dumps(manifest, indent=2))
print(json.dumps(dict(outputs=len(records), exact_pixel_pairs=len(pictures), skia_pixel_pairs=len(pictures))))
