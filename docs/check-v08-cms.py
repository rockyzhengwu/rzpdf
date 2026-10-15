"""Compare generated ICC inputs against independent LittleCMS and MuPDF."""
import json
import hashlib
from pathlib import Path
import subprocess
import tempfile
import pikepdf
from PIL import Image, ImageCms
ROOT=Path(__file__).resolve().parents[1]
OUT=Path(tempfile.mkdtemp(prefix="v08-cms-",dir=ROOT/"tmp/pdfs"))
print(OUT,flush=True)
def run(label,*args):
    result=subprocess.run([str(a) for a in args],capture_output=True,text=True)
    (OUT/f"{label}.log").write_text(result.stdout+result.stderr)
    assert result.returncode==0,(label,result.stderr)
    return result.stdout
run("generate",ROOT/"target/debug/examples/v08_cms",OUT)
run("qpdf","qpdf","--check",OUT/"cms.pdf")
destination=ImageCms.getOpenProfile(str(OUT/"srgb.icc"))
rows=[]
for line in (OUT/"samples.txt").read_text().splitlines():
    name,intent,encoded,*converted=line.split()
    samples=tuple(map(int,encoded.split(",")))
    mode={1:"L",3:"RGB",4:"CMYK"}[len(samples)]
    image=Image.new(mode,(1,1),samples[0] if mode=="L" else samples)
    reference=ImageCms.profileToProfile(image,str(OUT/f"{name}.icc"),destination,
        renderingIntent=["Perceptual","RelativeColorimetric","Saturation","AbsoluteColorimetric"].index(intent),outputMode="RGB").getpixel((0,0))
    actual=tuple(round(float(v)*255) for v in converted)
    error=max(abs(a-b) for a,b in zip(actual,reference))
    assert error<=2,(name,intent,actual,reference)
    rows.append(dict(profile=name,intent=intent,actual=actual,lcms=reference,max_channel_error=error))
run("skia",ROOT/"target/debug/examples/render_document",OUT/"cms.pdf",OUT/"skia")
run("mupdf","mutool","draw","-q","-r","72","-o",OUT/"mupdf-%d.png",OUT/"cms.pdf")
with pikepdf.open(OUT/"cms.pdf") as pdf:
    assert len(pdf.pages)==4
    for page in pdf.pages:
        assert page.Resources.ColorSpace.C[0]==pikepdf.Name.ICCBased
        assert str(page.Resources.XObject.Im.Intent)=="/RelativeColorimetric"
        assert str(page.Resources.ExtGState.R.RI)=="/RelativeColorimetric"
for index,name in enumerate(["srgb","adobe","gray","cmyk"],1):
    reference=next(r["lcms"] for r in rows if r["profile"]==name and r["intent"]=="RelativeColorimetric")
    for label,path in [("Skia",OUT/f"skia/skia-{index}.png"),("MuPDF",OUT/f"mupdf-{index}.png")]:
        image=Image.open(path).convert("RGB");assert image.size==(60,20)
        for x,route in [(10,"path"),(30,"image"),(50,"shading")]:
            actual=image.getpixel((x,10));error=max(abs(a-b) for a,b in zip(actual,reference))
            assert error<=2,(label,name,route,actual,reference)
extra=pikepdf.Pdf.new()
icc=pikepdf.Stream(extra,(OUT/"adobe.icc").read_bytes());icc.N=3
exponential=pikepdf.Dictionary(FunctionType=2,Domain=pikepdf.Array([0,1]),N=2,C0=pikepdf.Array([0,0,1]),C1=pikepdf.Array([1,0,0]))
calculator=pikepdf.Stream(extra,b"{ add 2 div dup 0 exch }");calculator.FunctionType=4;calculator.Domain=pikepdf.Array([0,1,0,1]);calculator.Range=pikepdf.Array([0,1,0,1,0,1])
cases=[
    (pikepdf.Array([pikepdf.Name.Lab,pikepdf.Dictionary(WhitePoint=pikepdf.Array([0.9642,1,0.8249]))]),"50.1960784314 0 0",bytes([128,128,128]),[0,100,-128,127,-128,127],(119,119,119)),
    (pikepdf.Array([pikepdf.Name.Separation,pikepdf.Name.Spot,pikepdf.Name.DeviceRGB,exponential]),str(128/255),bytes([128]),None,(64,0,191)),
    (pikepdf.Array([pikepdf.Name.DeviceN,pikepdf.Array([pikepdf.Name.A,pikepdf.Name.B]),pikepdf.Name.DeviceRGB,calculator]),f"{128/255} {128/255}",bytes([128,128]),None,(128,0,128)),
    (pikepdf.Array([pikepdf.Name.Indexed,pikepdf.Array([pikepdf.Name.ICCBased,icc]),0,pikepdf.String(bytes([51,128,204]))]),"0",bytes([0]),[0,255],next(r["lcms"] for r in rows if r["profile"]=="adobe" and r["intent"]=="RelativeColorimetric")),
]
for space,values,samples,decode,_ in cases:
    page=extra.add_blank_page(page_size=(40,20));image=pikepdf.Stream(extra,samples)
    image.Type=pikepdf.Name.XObject;image.Subtype=pikepdf.Name.Image;image.Width=1;image.Height=1;image.BitsPerComponent=8;image.ColorSpace=space
    if decode is not None:image.Decode=pikepdf.Array(decode)
    page.Resources=pikepdf.Dictionary(ColorSpace=pikepdf.Dictionary(C=space),XObject=pikepdf.Dictionary(Im=image))
    page.Contents=pikepdf.Stream(extra,f"/C cs {values} sc 0 0 20 20 re f q 20 0 0 20 20 0 cm /Im Do Q".encode())
extra.save(OUT/"nested-colours.pdf")
run("qpdf-nested","qpdf","--check",OUT/"nested-colours.pdf")
run("skia-nested",ROOT/"target/debug/examples/render_document",OUT/"nested-colours.pdf",OUT/"skia-nested")
run("mupdf-nested","mutool","draw","-q","-r","72","-o",OUT/"nested-%d.png",OUT/"nested-colours.pdf")
for index in [1,2]:
    # Freeze this project's explicit Alternate-only display policy rather than
    # MuPDF's named-colour/ink simulation. Keep original colourant rasters.
    page=extra.pages[index];expected=cases[index][-1]
    page.Resources.ColorSpace.C=pikepdf.Name.DeviceRGB
    image=page.Resources.XObject.Im;image.ColorSpace=pikepdf.Name.DeviceRGB;image.write(bytes(expected))
    values=" ".join(str(v/255) for v in expected)
    page.Contents=pikepdf.Stream(extra,f"/C cs {values} sc 0 0 20 20 re f q 20 0 0 20 20 0 cm /Im Do Q".encode())
extra.save(OUT/"alternate-policy-reference.pdf")
run("mupdf-alternate-policy","mutool","draw","-q","-r","72","-o",OUT/"alternate-%d.png",OUT/"alternate-policy-reference.pdf")
nested=[]
for index,case in enumerate(cases,1):
    for label,path in [("Skia",OUT/f"skia-nested/skia-{index}.png"),("MuPDF-matched-policy",OUT/f"alternate-{index}.png")]:
        image=Image.open(path).convert("RGB");assert image.size==(40,20)
        for x in [10,30]:
            actual=image.getpixel((x,10));error=max(abs(a-b) for a,b in zip(actual,case[-1]))
            assert error<=2,("nested",label,index,x,actual,case[-1]);nested.append(dict(reader=label,page=index,x=x,max_channel_error=error))
report=dict(source_head=run("head","git","-C",ROOT,"rev-parse","HEAD").strip(),
    working_tree=run("status","git","-C",ROOT,"status","--short"),
    lcms_version=ImageCms.core.littlecms_version,transforms=rows,nested_colours=nested,
    profile_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(OUT.glob("*.icc"))},
    colourant_reference_policy="explicit tint-function -> RGB Alternate; original MuPDF ink simulation rasters retained")
(OUT/"report.json").write_text(json.dumps(report,indent=2)+"\n")
print("PASS",OUT,flush=True)
