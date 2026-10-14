"""Independent v0.6 pixels, sources, shared Globals, R6 and rendering checks.

Requires built v06_images/render_document, Pillow, PyMuPDF, pikepdf and qpdf.
PYTHONPATH=tmp/pdfs/v06-verifiers:tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v06-images.py
Evidence is written into a new directory; previous output is never replaced.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

import fitz
import pikepdf
from PIL import Image, ImageChops

ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/"pdf/tests/fixtures/v06"
EVIDENCE=Path(tempfile.mkdtemp(prefix="v06-images-",dir=ROOT/"tmp/pdfs"))
OUTPUT=EVIDENCE/"rzpdf"
started=time.monotonic()
def run(*args):
    result=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    if result.returncode:
        raise RuntimeError(f"exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip() or result.stderr.strip()

for name,digest in json.loads((FIXTURE/"manifest.json").read_text()).items():
    assert hashlib.sha256((FIXTURE/name).read_bytes()).hexdigest()==digest,name
run(ROOT/"target/debug/examples/v06_images",FIXTURE/"external-codecs.pdf",OUTPUT)

def checked(path,password=""):
    with pikepdf.Pdf.open(path,password=password,attempt_recovery=False) as pdf:
        assert not pdf.check_pdf_syntax(),path
    run("qpdf",f"--password={password}","--check",path)

sources={}
for label,path in [("external",FIXTURE/"external-codecs.pdf"),("roundtrip",OUTPUT/"roundtrip.pdf"),("imported",OUTPUT/"imported.pdf")]:
    checked(path)
    with pikepdf.Pdf.open(path) as pdf:
        sources[label]=[page.Resources.XObject.Im.read_raw_bytes() for page in pdf.pages]
        globals_refs=[pdf.pages[index].Resources.XObject.Im.DecodeParms.JBIG2Globals for index in (6,7)]
        assert globals_refs[0].objgen==globals_refs[1].objgen
        assert globals_refs[0].read_raw_bytes()==(FIXTURE/"patterns-global.jb2").read_bytes()
assert sources["external"]==sources["roundtrip"]==sources["imported"]

pixels=[]
with fitz.open(FIXTURE/"external-codecs.pdf") as pdf:
    rows=[row.split("\t") for row in (OUTPUT/"images.tsv").read_text().splitlines()]
    assert len(rows)==9
    for page,draw,width,height,filename in rows:
        index=int(page)-1
        pix=fitz.Pixmap(pdf,pdf[index].get_images()[0][0])
        if pix.colorspace.n!=3:
            pix=fitz.Pixmap(fitz.csRGB,pix)
        rgba=Image.frombytes("RGB",(pix.width,pix.height),pix.samples).convert("RGBA").tobytes()
        ours=(OUTPUT/filename).read_bytes()
        assert (pix.width,pix.height)==(int(width),int(height))
        delta=max(abs(a-b) for a,b in zip(ours,rgba))
        assert len(ours)==len(rgba)
        tolerance=2 if index==2 else 0
        assert delta<=tolerance,(index,delta,tolerance)
        pixels.append({"page":index+1,"max_sample_error":delta,"tolerance":tolerance,"sha256":hashlib.sha256(ours).hexdigest()})
        pix.save(EVIDENCE/f"decoded-{index+1}.png")

def render(path,label,password=""):
    args=["mutool","draw","-q","-r","72","-o",EVIDENCE/f"{label}-%d.png"]
    if password:
        args.extend(["-p",password])
    run(*args,path)
    return [Image.open(p).convert("RGBA").tobytes() for p in sorted(EVIDENCE.glob(f"{label}-*.png"))]

baseline=render(FIXTURE/"external-codecs.pdf","external")
assert render(OUTPUT/"roundtrip.pdf","roundtrip")==baseline
assert render(OUTPUT/"imported.pdf","imported")==baseline
for name in ("inline","inline-replaced","inline-removed"):
    checked(OUTPUT/f"{name}.pdf")
    render(OUTPUT/f"{name}.pdf",name)
checked(OUTPUT/"inline-r6.pdf","user")
assert render(OUTPUT/"inline-r6.pdf","inline-r6","user")==render(OUTPUT/"inline.pdf","inline-repeat")
# Neighbour draw remains byte-identical in a disjoint rectangle after edits.
original=Image.open(EVIDENCE/"inline-1.png").convert("RGB")
for name in ("inline-replaced","inline-removed"):
    after=Image.open(EVIDENCE/f"{name}-1.png").convert("RGB")
    assert original.crop((200,232,260,292)).tobytes()==after.crop((200,232,260,292)).tobytes()
assert original.getpixel((60,202))==(255,0,0)
assert Image.open(EVIDENCE/"inline-replaced-1.png").convert("RGB").getpixel((60,202))==(0,255,255)
assert Image.open(EVIDENCE/"inline-removed-1.png").convert("RGB").crop((30,172,150,292)).getextrema()==((255,255),(255,255),(255,255))

# Exercise the actual production rendering path, not only independent images.
run(ROOT/"target/debug/examples/render_document",FIXTURE/"external-codecs.pdf",EVIDENCE/"skia")
run(ROOT/"target/debug/examples/render_document",OUTPUT/"inline.pdf",EVIDENCE/"skia-inline")
for index in range(9):
    skia=Image.open(EVIDENCE/"skia"/f"skia-{index+1}.png").convert("RGB")
    mupdf=Image.open(EVIDENCE/f"external-{index+1}.png").convert("RGB")
    assert skia.size==mupdf.size
    expected=Image.new("RGB",skia.size,"white")
    decoded=Image.open(EVIDENCE/f"decoded-{index+1}.png").convert("RGB")
    expected.paste(decoded.resize((64,64),Image.Resampling.NEAREST),(18,18))
    assert skia.tobytes()==expected.tobytes(),("nearest full-page pixels",index)
    background=Image.new("RGB",skia.size,"white")
    assert ImageChops.difference(skia,background).getbbox()==(18,18,82,82)
    assert ImageChops.difference(mupdf,background).getbbox()==(18,18,82,82)
    # MuPDF samples differently near colour discontinuities and boundaries.
    # Exact decoded samples and nearest full-page comparison are the pixel gates.
    # These well-interior samples supplement the render geometry comparison.
    for x,y in [(20,20),(40,40),(60,60)]:
        assert max(abs(a-b) for a,b in zip(skia.getpixel((x,y)),mupdf.getpixel((x,y))))<=4,(index,x,y)

report={"source_commit":run("git","-C",ROOT,"rev-parse","HEAD"),"versions":{"mupdf":run("mutool","-v"),"qpdf":run("qpdf","--version"),"pikepdf":pikepdf.__version__,"libqpdf":pikepdf.__libqpdf_version__,"pymupdf":fitz.VersionBind},"pixels":pixels,"elapsed_seconds":round(time.monotonic()-started,3),"evidence":str(EVIDENCE)}
(EVIDENCE/"report.json").write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps(report,indent=2))
