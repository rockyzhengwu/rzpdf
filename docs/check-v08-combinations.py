"""Independent nested/combined paints and 16 blend modes on path/image/text."""
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image,ImageChops,ImageStat
ROOT=Path(__file__).resolve().parents[1]
OUT=Path(tempfile.mkdtemp(prefix="v08-combinations-",dir=ROOT/"tmp/pdfs"));print(OUT,flush=True)
def run(label,*args):
    p=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    (OUT/f"{label}.log").write_text(p.stdout+p.stderr);assert p.returncode==0,(label,p.stderr);return p.stdout
run("generate",ROOT/"target/debug/examples/v08_combinations",OUT/"combinations.pdf")
run("qpdf","qpdf","--check",OUT/"combinations.pdf")
with pikepdf.open(OUT/"combinations.pdf") as pdf:
    assert len(pdf.pages)==22
    for page in pdf.pages[6:]:
        image=next(v for _,v in page.Resources.XObject.items())
        assert set(image.SMask.read_bytes())=={255}
        # An all-opaque image SMask is exactly equivalent to no image SMask.
        # MuPDF 1.27 loses ca for this all-opaque-mask fast path. Preserve raw.
        del image["/SMask"]
    for page in pdf.pages[4:6]:
        # These particular patterns cover the page with one constant gray
        # Multiply source. The direct equivalent independently checks the
        # backdrop-dependent result; MuPDF isolates even non-Normal patterns.
        page.Resources.ExtGState=pikepdf.Dictionary(M=pikepdf.Dictionary(BM=pikepdf.Name.Multiply))
        page.Contents=pikepdf.Stream(pdf,b"0 1 0 rg 0 0 60 40 re f /M gs 0.5 g 0 0 60 40 re f")
    pdf.save(OUT/"equivalent-reference.pdf")
    pdf.pages[12].Resources.ExtGState.M.ca=1
    pdf.save(OUT/"opaque-dodge.pdf")
rows=[]
for dpi in [72,144]:
    run(f"skia-{dpi}",ROOT/"target/debug/examples/render_document",OUT/"combinations.pdf",OUT/f"skia-{dpi}",dpi,dpi)
    run(f"mupdf-{dpi}","mutool","draw","-q","-r",dpi,"-o",OUT/f"mupdf-{dpi}-%d.png",OUT/"combinations.pdf")
    run(f"mupdf-equivalent-{dpi}","mutool","draw","-q","-r",dpi,"-o",OUT/f"equivalent-{dpi}-%d.png",OUT/"equivalent-reference.pdf")
    run(f"mupdf-opaque-dodge-{dpi}","mutool","draw","-q","-r",dpi,"-o",OUT/f"opaque-dodge-{dpi}-%d.png",OUT/"opaque-dodge.pdf","13")
    scale=dpi//72
    for page in range(1,23):
        a=Image.open(OUT/f"skia-{dpi}/skia-{page}.png").convert("RGB");original=Image.open(OUT/f"mupdf-{dpi}-{page}.png").convert("RGB")
        b=Image.open(OUT/f"equivalent-{dpi}-{page}.png").convert("RGB")
        assert a.size==b.size==(60*scale,40*scale)
        delta=ImageChops.difference(a,b);delta.save(OUT/f"difference-{dpi}-{page}.png");mean=sum(ImageStat.Stat(delta).mean)/3
        if page!=13: assert mean<=2,(dpi,page,mean)
        points=[(20,20)] if page<=3 else [(5,20),(10,20),(20,20),(45,20)] if page==4 else [(5,20),(25,20),(55,20)] if page<=6 else [(10,20),(30,20),(45,20)]
        for x,y in points:
            point=(x*scale,y*scale)
            if page==13:
                # MuPDF's premultiplied 8-bit ColorDodge can amplify a one-unit
                # unpremultiplication error. Independently check the blend at
                # alpha=1, then PDF's alpha interpolation at alpha=1/2.
                opaque=Image.open(OUT/f"opaque-dodge-{dpi}-13.png").convert("RGB").getpixel(point)
                expected=tuple(round((s+back)/2) for s,back in zip(opaque,[51,153,102]))
                assert max(abs(v-w) for v,w in zip(a.getpixel(point),[153,172,179]))<=1
                assert max(abs(v-w) for v,w in zip(a.getpixel(point),expected))<=2,(dpi,page,point,a.getpixel(point),expected)
            else: assert max(abs(v-w) for v,w in zip(a.getpixel(point),b.getpixel(point)))<=2,(dpi,page,point,a.getpixel(point),b.getpixel(point))
        if page<=3:
            expected=[(191,0,0),(64,191,64),(0,191,0)][page-1]
            assert max(abs(v-w) for v,w in zip(a.getpixel((20*scale,20*scale)),expected))<=2,(page,expected)
        if page in [5,6]: assert a.getpixel((25*scale,20*scale))==(0,128,0)
        rows.append(dict(dpi=dpi,page=page,mean_channel_difference=mean,original_mupdf_mean_difference=sum(ImageStat.Stat(ImageChops.difference(a,original)).mean)/3))
(OUT/"report.json").write_text(json.dumps(dict(source_head=run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),working_tree=run("status","git","-C",ROOT,"status","--short"),pages=rows),indent=2)+"\n")
print("PASS",OUT,flush=True)
