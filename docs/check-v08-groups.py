"""RGB isolated/nonisolated group gate; knockout and masks remain separate."""
import json
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image, ImageChops, ImageStat
ROOT=Path(__file__).resolve().parents[1]
OUT=Path(tempfile.mkdtemp(prefix="v08-groups-",dir=ROOT/"tmp/pdfs"))
print(OUT,flush=True)
def run(label,*args):
    result=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    (OUT/f"{label}.log").write_text(result.stdout+result.stderr)
    assert result.returncode==0,(label,result.returncode,result.stderr)
    return result.stdout
run("generate",ROOT/"target/debug/examples/v08_groups",OUT/"groups.pdf")
run("qpdf","qpdf","--check",OUT/"groups.pdf")
with pikepdf.open(OUT/"groups.pdf") as pdf:
    assert len(pdf.pages)==10
    for index,page in enumerate(pdf.pages):
        form=next(v for _,v in page.Resources.XObject.items())
        assert bool(form.Group.I)==(index not in [6,8])
        if index<5: assert not form.Group.K and str(form.Group.CS)=="/DeviceRGB"
    # MuPDF 1.27 optimizes ca=0 members away, losing their knockout shape.
    # Keep that original raster; a one-alpha-unit neighbor independently
    # witnesses the same shape without the optimization. It is NOT called an
    # equivalent PDF: its opacity differs by exactly 1/255, bounded below.
    for index in [7,8]:
        form=next(v for _,v in pdf.pages[index].Resources.XObject.items())
        form.Resources.ExtGState.Zero.ca=1/255
    pdf.save(OUT/"one-alpha-neighbor.pdf")
run("skia",ROOT/"target/debug/examples/render_document",OUT/"groups.pdf",OUT/"skia")
run("mupdf","mutool","draw","-q","-r","72","-o",OUT/"mupdf-%d.png",OUT/"groups.pdf")
run("mupdf-neighbor","mutool","draw","-q","-r","72","-o",OUT/"neighbor-%d.png",OUT/"one-alpha-neighbor.pdf","8-9")
rows=[]
for page in range(1,11):
    actual=Image.open(OUT/f"skia/skia-{page}.png").convert("RGB")
    reference=Image.open(OUT/f"mupdf-{page}.png").convert("RGB")
    assert actual.size==reference.size==(60,40)
    delta=ImageChops.difference(actual,reference)
    delta.save(OUT/f"difference-{page}.png")
    raw_mean=sum(ImageStat.Stat(delta).mean)/3
    if page in [8,9]:
        assert reference.getpixel((25,20))==(255,0,0)
        neighbor=Image.open(OUT/f"neighbor-{page}.png").convert("RGB")
        # Outside that one strip, the original readers must still agree.
        assert ImageChops.difference(reference.crop((0,0,20,40)),neighbor.crop((0,0,20,40))).getbbox() is None
        assert ImageChops.difference(reference.crop((30,0,60,40)),neighbor.crop((30,0,60,40))).getbbox() is None
        reference=neighbor;delta=ImageChops.difference(actual,reference)
    mean=sum(ImageStat.Stat(delta).mean)/3
    assert mean<=2.0,(page,mean)
    # Each reader is checked against compositing arithmetic, rather than
    # assuming their two independent 8-bit rounding paths coincide. A pairwise
    # difference can therefore reach four when each has error at most two.
    alpha,blend=[(0.,"Normal"),(.25,"Normal"),(.5,"Normal"),(.75,"Multiply"),(1.,"Screen"),(.5,"Normal"),(.5,"Normal"),(1.,"Normal"),(1.,"Normal"),(1.,"Normal")][page-1]
    errors=[]
    points=[(10,20),(25,20),(42,20),(47,20),(55,20)] if page<8 else [(5,20),(15,20),(25,20),(35,20),(55,20)]
    for point in points:
        x,_=point
        if page>=8:
            expected={5:(255,0,0),15:((128,0,128) if page==10 else (0,128,128)),25:((255,0,0) if page==10 else (0,255,0)),35:((191,0,0) if page==10 else (64,191,64) if page==8 else (0,191,0)),55:(0,255,0)}[x]
        elif x==55: expected=(255,255,0)
        elif x==47: expected=(0,255,0)
        else:
            if page>=6: color,opacity=((.5,.5,.5),1.) if page==6 else ((0.,.5,0.),1.)
            else: color,opacity={10:((1.,0.,0.),1.),25:((.5,0.,0.),1.),42:((0.,0.,1.),.5)}[x]
            backdrop=(0.,1.,0.)
            if blend=="Normal": mixed=color
            elif blend=="Multiply": mixed=tuple(a*b for a,b in zip(color,backdrop))
            else: mixed=tuple(a+b-a*b for a,b in zip(color,backdrop))
            weight=alpha*opacity
            expected=tuple(int(255*((1-weight)*b+weight*c)+.5) for b,c in zip(backdrop,mixed))
        for label,image in [("Skia",actual),("MuPDF",reference)]:
            error=max(abs(a-b) for a,b in zip(image.getpixel(point),expected))
            assert error<=2,(page,point,label,image.getpixel(point),expected)
            errors.append(error)
    row={"page":page,"mean_channel_difference":mean,"original_mupdf_mean_difference":raw_mean,"comparison":"one-alpha-unit neighbor" if page in [8,9] else "original","analytic_max_channel_error":max(errors)}
    rows.append(row);print(row,flush=True)
report={"source_head":run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),"working_tree":run("status","git","-C",ROOT,"status","--short"),"pages":rows}
(OUT/"report.json").write_text(json.dumps(report,indent=2)+"\n")
print("PASS",OUT,flush=True)
