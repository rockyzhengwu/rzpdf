"""Graphics Alpha/Luminosity mask gate with arithmetic and independent raster."""
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image,ImageChops,ImageStat,ImageFilter
ROOT=Path(__file__).resolve().parents[1]
OUT=Path(tempfile.mkdtemp(prefix="v08-masks-",dir=ROOT/"tmp/pdfs"))
print(OUT,flush=True)
def run(label,*args):
    result=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    (OUT/f"{label}.log").write_text(result.stdout+result.stderr)
    assert result.returncode==0,(label,result.returncode,result.stderr)
    return result.stdout
run("generate",ROOT/"target/debug/examples/v08_masks",OUT/"masks.pdf")
run("qpdf","qpdf","--check",OUT/"masks.pdf")
with pikepdf.open(OUT/"masks.pdf") as pdf:
    assert len(pdf.pages)==10
    for page in pdf.pages: assert str(page.Resources.ExtGState.M.SMask.S) in ["/Alpha","/Luminosity"]
    opaque=pikepdf.Stream(pdf,b"0 0 60 40 re f")
    opaque.Subtype=pikepdf.Name("/Form")
    opaque.BBox=pikepdf.Array([0,0,60,40])
    opaque.Resources=pikepdf.Dictionary()
    opaque.Group=pikepdf.Dictionary(S=pikepdf.Name("/Transparency"),I=True,CS=pikepdf.Name("/DeviceRGB"))
    pdf.pages[5].Resources.ExtGState.M.SMask=pikepdf.Dictionary(S=pikepdf.Name("/Alpha"),G=opaque)
    pdf.save(OUT/"opaque-mask-text.pdf")
run("skia",ROOT/"target/debug/examples/render_document",OUT/"masks.pdf",OUT/"skia")
run("mupdf-default-icc","mutool","draw","-q","-r","72","-o",OUT/"mupdf-default-%d.png",OUT/"masks.pdf")
# This gate freezes nominal DeviceRGB luminosity, not an ICC-managed RGB->Gray
# conversion. Retain default-ICC rasters, but use the explicitly matched policy.
run("mupdf-device-rgb","mutool","draw","-q","-N","-r","72","-o",OUT/"mupdf-%d.png",OUT/"masks.pdf")
run("skia-opaque-mask",ROOT/"target/debug/examples/render_document",OUT/"opaque-mask-text.pdf",OUT/"skia-opaque-mask")
run("mupdf-opaque-mask","mutool","draw","-q","-N","-r","72","-o",OUT/"mupdf-opaque-mask-%d.png",OUT/"opaque-mask-text.pdf","6")
rows=[]
for page in range(1,11):
    actual=Image.open(OUT/f"skia/skia-{page}.png").convert("RGB")
    reference=Image.open(OUT/f"mupdf-{page}.png").convert("RGB")
    assert actual.size==reference.size==(60,40)
    difference=ImageChops.difference(actual,reference);difference.save(OUT/f"difference-{page}.png")
    mean=sum(ImageStat.Stat(difference).mean)/3
    if page!=6: assert mean<=2.0,(page,mean)
    error=0
    if page in (1,2,3,4,10):
        masks={1:[1.,.5,0.,0.],2:[1.,.25,0.,0.],3:[.3,.295,0.,0.],4:[.3,.795,1.,1.],10:[.3,.545,.5,.5]}[page]
        for x,alpha in zip([5,20,40,55],masks):
            expected=(int(255*(1-alpha)+.5),)*2+(255,)
            for label,image in [("Skia",actual),("MuPDF",reference)]:
                delta=max(abs(a-b) for a,b in zip(image.getpixel((x,20)),expected));error=max(error,delta)
                assert delta<=2,(page,x,label,image.getpixel((x,20)),expected)
    if page==6:
        # Different font rasterizers need not choose identical edge coverage.
        # Verify mask arithmetic against each backend's fully opaque-mask text,
        # preserving shader-driven glyph antialiasing (None may enable LCD).
        # then separately require their glyph silhouettes to agree within 1px.
        base_skia=Image.open(OUT/"skia-opaque-mask/skia-6.png").convert("RGB")
        base_mupdf=Image.open(OUT/"mupdf-opaque-mask-6.png").convert("RGB")
        for label,image,base in [("Skia",actual,base_skia),("MuPDF",reference,base_mupdf)]:
            for y in range(40):
                for x in range(60):
                    alpha=1. if x<15 else (.5 if x<30 else 0.)
                    expected=tuple(int(255-alpha*(255-v)+.5) for v in base.getpixel((x,y)))
                    delta=max(abs(a-b) for a,b in zip(image.getpixel((x,y)),expected));error=max(error,delta)
                    assert delta<=2,(page,label,x,y,image.getpixel((x,y)),expected)
        silhouettes=[base.getchannel("R").point(lambda v:255 if v<250 else 0) for base in [base_skia,base_mupdf]]
        assert ImageChops.subtract(silhouettes[0],silhouettes[1].filter(ImageFilter.MaxFilter(3))).getbbox() is None
        assert ImageChops.subtract(silhouettes[1],silhouettes[0].filter(ImageFilter.MaxFilter(3))).getbbox() is None
    row={"page":page,"mean_channel_difference":mean,"analytic_max_channel_error":error if page in (1,2,3,4,6,10) else None,
         "glyph_edge_tolerance_pixels":1 if page==6 else None}
    rows.append(row);print(row,flush=True)
report={"source_head":run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),"working_tree":run("status","git","-C",ROOT,"status","--short"),"reference_color_policy":"nominal DeviceRGB, MuPDF -N; default ICC rasters retained separately","pages":rows}
(OUT/"report.json").write_text(json.dumps(report,indent=2)+"\n")
print("PASS",OUT,flush=True)
