"""Offline synthetic recovery gate. Existing pikepdf, MuPDF, Skia and Pillow only.
Build render --example v012_recovery first; use a NEW output directory.
No recovered PDF is written: only detached PNGs and diagnostic logs.
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
import pikepdf
from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXE = ROOT / "target/debug/examples/v012_recovery"
records, negatives, pictures = [], [], []

def run(label, *args, success=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr

def damage_start(data):
    return re.sub(rb"startxref\s+\d+\s+%%EOF\s*$", b"startxref\n0\n%%EOF\n", data)

seed = OUT / "pikepdf-classic.pdf"
pdf = pikepdf.Pdf.new()
for index in range(2):
    page = pdf.add_blank_page(page_size=(320, 240))
    page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F1=pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type1, BaseFont=pikepdf.Name.Helvetica))))
    page.Contents = pdf.make_stream(f"BT /F1 20 Tf 25 180 Td (Recovery page {index+1}) Tj ET\n0 0 1 rg 25 30 50 35 re f\n% 999 0 obj trailer << /Root 999 0 R >> endobj\n".encode())
pdf.docinfo.Title = "Synthetic recovery fixture"
pdf.save(seed, force_version="1.7", compress_streams=False, object_stream_mode=pikepdf.ObjectStreamMode.disable)
qpdf_seed = OUT / "qpdf-classic.pdf"
run("qpdf-produce", "qpdf", "--object-streams=disable", "--stream-data=compress", seed, qpdf_seed)
mutool_seed = OUT / "mupdf-classic.pdf"
run("mupdf-produce", "mutool", "clean", seed, mutool_seed)

for original in [seed, qpdf_seed, mutool_seed]:
    name = original.stem
    run(name+"-check", "qpdf", "--check", original)
    run(name+"-strict", EXE, original, OUT / (name+"-strict"), "strict")
    text = run(name+"-reference", "mutool", "draw", "-r", "72", "-o", OUT / (name+"-mupdf-%d.png"), original)
    assert not re.search(r"repair|reconstruct|format error|cannot load", text, re.I)
    data = original.read_bytes()
    match = re.search(rb"startxref\s+(\d+)\s+%%EOF\s*$", data)
    assert match
    xref = int(match[1])
    trailer = data.find(b"trailer", xref)
    assert trailer > xref
    # Each mutation retains all object bytes and one explicit trailer Root.
    variants = {
        "bad-start": damage_start(data),
        "missing-table": damage_start(data[:xref] + data[trailer:]),
        "bad-table": damage_start(data[:xref] + b"xref\n0 1\n0 65535 f\n" + data[trailer:]),
    }
    for variant, damaged in variants.items():
        label = name+"-"+variant
        input_file = OUT / (label+".pdf")
        input_file.write_bytes(damaged)
        run(label+"-strict-refusal", EXE, input_file, OUT / (label+"-forbidden"), "strict", success=False)
        assert not (OUT / (label+"-forbidden")).exists()
        report = run(label+"-recover", EXE, input_file, OUT / label, "recover")
        assert "RebuiltReadOnly" in report and "RebuiltObjectIndex" in report
        for page in [1,2]:
            recovered = OUT / label / f"page-{page}.png"
            with Image.open(recovered).convert("RGBA") as a, Image.open(OUT / (name+"-strict") / f"page-{page}.png").convert("RGBA") as b:
                assert a.size == b.size and ImageChops.difference(a,b).getbbox() is None
            pictures.append((label+f" / {page}", recovered))
        records.append({"case": label,"producer":name,"source_sha256":hashlib.sha256(data).hexdigest(),"damaged_sha256":hashlib.sha256(damaged).hexdigest(),"pages":2,"assertion":"Strict refuses; detached Skia pixels equal undamaged Strict source"})

base = variants["missing-table"]
position = base.find(b"trailer")
for name, extra in {
    "duplicate-object":b"1 0 obj null endobj\n",
    "signature":b"20 0 obj << /Type /Sig /ByteRange [0 1 2 3] >> endobj\n",
    "object-stream":b"20 0 obj << /Type /ObjStm /Length 0 /N 0 /First 0 >> stream\n\nendstream endobj\n",
    "unknown-bytes":b"ambiguous damaged bytes\n",
    "historical-eof":b"%%EOF\n",
}.items():
    bad = OUT / ("negative-"+name+".pdf")
    bad.write_bytes(base[:position] + extra + base[position:])
    run("negative-"+name, EXE,bad,OUT/("negative-"+name),"recover",success=False)
    assert not (OUT/("negative-"+name)).exists()
    negatives.append(name)
for name, key in [("encrypted",b"/Encrypt null"),("history",b"/Prev 0"),("hybrid",b"/XRefStm 1"),("duplicate-root",b"/Root 1 0 R /Root 2 0 R")]:
    bad=OUT/("negative-"+name+".pdf")
    bad.write_bytes(base.replace(b"trailer\n<<",b"trailer\n<< "+key+b" ",1))
    run("negative-"+name,EXE,bad,OUT/("negative-"+name),"recover",success=False)
    negatives.append(name)

for start in range(0,len(pictures),12):
    sheet=Image.new("RGB",(1200,4*270),"#eeeeee")
    draw=ImageDraw.Draw(sheet)
    for slot,(label,png) in enumerate(pictures[start:start+12]):
        im=Image.open(png).convert("RGB");im.thumbnail((390,240))
        x,y=(slot%3)*400,(slot//3)*270
        draw.text((x+4,y+3),label,fill="black");sheet.paste(im,(x+4,y+24))
    sheet.save(OUT/f"visual-review-{start//12+1}.png")
manifest={"cases":records,"negative_cases":negatives,"exact_skia_pairs":len(pictures),"mupdf_reference_pages":6,"license":"Original synthetic generation recipe under repository AGPL-3.0-only; no third-party PDFs copied","versions":{"pikepdf":pikepdf.__version__,"qpdf":run("qpdf-version","qpdf","--version"),"mupdf":run("mutool-version","mutool","-v")}}
(OUT/"manifest.json").write_text(json.dumps(manifest,indent=2))
print(f"PASS cases={len(records)} negatives={len(negatives)} exact_pairs={len(pictures)} mupdf_reference_pages=6")
