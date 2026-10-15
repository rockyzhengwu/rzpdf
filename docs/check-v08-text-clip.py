"""Compare glyph-outline geometry independently, separating edge rasterization."""
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image,ImageChops,ImageStat,ImageFilter
ROOT=Path(__file__).resolve().parents[1]
OUT=Path(tempfile.mkdtemp(prefix="v08-text-clip-",dir=ROOT/"tmp/pdfs"))
print(OUT,flush=True)
def run(label,*args):
    result=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    (OUT/f"{label}.log").write_text(result.stdout+result.stderr)
    assert result.returncode==0,(label,result.stderr)
    return result.stdout
run("generate",ROOT/"target/debug/examples/v08_text_clip",OUT/"clip.pdf")
run("qpdf","qpdf","--check",OUT/"clip.pdf")
with pikepdf.open(OUT/"clip.pdf") as pdf: assert len(pdf.pages)==9
run("skia",ROOT/"target/debug/examples/render_document",OUT/"clip.pdf",OUT/"skia","144","144")
run("mupdf","mutool","draw","-q","-r","144","-o",OUT/"mupdf-%d.png",OUT/"clip.pdf")
rows=[]
for page in range(1,10):
    a=Image.open(OUT/f"skia/skia-{page}.png").convert("RGB");b=Image.open(OUT/f"mupdf-{page}.png").convert("RGB")
    assert a.size==b.size==(80,80)
    delta=ImageChops.difference(a,b);delta.save(OUT/f"difference-{page}.png")
    # Interior red clip coverage cannot be excused as a font rasterizer delta.
    def mask(im):
        return Image.frombytes("L",im.size,bytes(255 if r>180 and g<80 and blue<80 else 0 for r,g,blue in im.getdata()))
    ma,mb=mask(a),mask(b)
    for foreground,reference in [(ma,mb),(mb,ma)]:
        residual=ImageChops.subtract(foreground,reference.filter(ImageFilter.MaxFilter(3)))
        assert residual.getbbox() is None,(page,"clip geometry differs beyond one output pixel")
    if page==7: assert ma.getbbox() is None and mb.getbbox() is None
    mean=sum(ImageStat.Stat(delta).mean)/3
    assert mean<5,(page,mean)
    rows.append(dict(page=page,mean_channel_difference=mean,red_clip_bounds=ma.getbbox()))
(OUT/"report.json").write_text(json.dumps(dict(source_head=run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),working_tree=run("status","git","-C",ROOT,"status","--short"),pages=rows),indent=2)+"\n")
print("PASS",OUT,flush=True)
