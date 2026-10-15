"""Retest existing Type1-5 fixed corpus using independent pixel centers."""
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image,ImageChops,ImageStat
ROOT=Path(__file__).resolve().parents[1]
OUT=Path(tempfile.mkdtemp(prefix="v08-shadings-",dir=ROOT/"tmp/pdfs"));print(OUT,flush=True)
def run(label,*args):
    p=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    (OUT/f"{label}.log").write_text(p.stdout+p.stderr);assert p.returncode==0,(label,p.stderr);return p.stdout
run("generate",ROOT/"target/debug/examples/create_shading",OUT/"shadings.pdf")
run("qpdf","qpdf","--check",OUT/"shadings.pdf")
with pikepdf.open(OUT/"shadings.pdf") as pdf:
    assert len(pdf.pages)==14
    for page in pdf.pages:
        for _,shade in page.Resources.get("/Shading",{}).items():
            if "/Background" in shade: del shade["/Background"]
    pdf.save(OUT/"direct-sh-standard.pdf")
run("skia",ROOT/"target/debug/examples/render_document",OUT/"shadings.pdf",OUT/"skia")
run("mupdf-original","mutool","draw","-q","-r","72","-o",OUT/"mupdf-original-%d.png",OUT/"shadings.pdf")
run("mupdf","mutool","draw","-q","-r","576","-o",OUT/"mupdf-%d.png",OUT/"direct-sh-standard.pdf")
rows=[]
for page in range(1,15):
    actual=Image.open(OUT/f"skia/skia-{page}.png").convert("RGB")
    reference=Image.open(OUT/f"mupdf-{page}.png").convert("RGB").resize(actual.size,Image.Resampling.NEAREST)
    assert actual.size==reference.size==((120,200) if page==5 else (200,120))
    delta=ImageChops.difference(actual,reference);delta.save(OUT/f"difference-{page}.png")
    mean=sum(ImageStat.Stat(delta).mean)/3;assert mean<=2,(page,mean)
    if page>=8:
        expected=(128,126,127) if page==8 else (128,128,128) if page in [9,12] else (64,64,64)
        # PDF pixel center (100.5,59.5): for mesh x interpolates at 100.5
        # or 99.5 on a mirrored page, rather than the edge at exactly 100.
        for image in [actual,reference]: assert max(abs(v-w) for v,w in zip(image.getpixel((100,60)),expected))<=2,(page,image.getpixel((100,60)),expected)
    rows.append(dict(page=page,mean_channel_difference=mean))
(OUT/"report.json").write_text(json.dumps(dict(source_head=run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),working_tree=run("status","git","-C",ROOT,"status","--short"),pages=rows),indent=2)+"\n")
print("PASS",OUT,flush=True)
