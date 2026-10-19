"""Synthetic OCG authoring gate, AGPL-3.0-only. Existing tools; new directory.
Build pdf --example v013_layers and render --example render_optional_content.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path
import pikepdf
from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
records, pictures = [], []

def run(label, *args):
    p = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240)
    (OUT / (label + '.log')).write_text(p.stdout + p.stderr)
    assert p.returncode == 0, (label, p.returncode, p.stderr)
    return p.stdout + p.stderr

seed = OUT / 'pikepdf.pdf'
with pikepdf.Pdf.new() as pdf:
    for _ in range(2):
        page = pdf.add_blank_page(page_size=(200, 200))
        page.Contents = pdf.make_stream(b'q 0 g 10 10 20 20 re f Q\n')
    pdf.save(seed, force_version='2.0')
recipe = OUT / 'mupdf-page.txt'
recipe.write_text('%%MediaBox 0 0 200 200\nq 0 g 10 10 20 20 re f Q\n')
mutool = OUT / 'mupdf.pdf'
run('mupdf-create', 'mutool', 'create', '-o', mutool, recipe, recipe)

def expected(label, red, blue):
    path = OUT / (label + '-expected.pdf')
    with pikepdf.Pdf.new() as pdf:
        for page_no in range(2):
            page = pdf.add_blank_page(page_size=(200, 200))
            data = b'q 0 g 10 10 20 20 re f Q\n'
            if red: data += b'1 0 0 rg 10 100 40 40 re f\n'
            if blue: data += b'0 0 1 rg 70 100 40 40 re f\n'
            if red and not blue: data += b'1 0.5 0 rg 10 60 40 20 re f\n'
            if page_no == 1 or red: data += b'0 1 0 rg 130 100 30 30 re f\n'
            page.Contents = pdf.make_stream(data)
        pdf.save(path)
    run(label + '-expected-render', 'mutool', 'draw', '-q', '-r', '72', '-o', OUT / (label + '-expected-%d.png'), path)

for label, selection, red, blue in [('default','default',True,False),('swap','0',False,True),('both','1',True,True)]:
    expected(label,red,blue)

for source in [seed,mutool]:
    producer = source.stem
    run(producer+'-input-check','qpdf','--check',source)
    native_out=OUT/(producer+'-authored')
    run(producer+'-author',ROOT/'target/debug/examples/v013_layers',source,native_out)
    for authored in sorted(native_out.glob('*.pdf')):
        label=producer+'-'+authored.stem
        encrypted=authored.stem.endswith('-r6')
        run(label+'-check','qpdf',*(['--password=user'] if encrypted else []),'--check',authored)
        with pikepdf.open(authored,password='user' if encrypted else '') as pdf:
            props=pdf.Root.OCProperties
            assert len(props.OCGs)==2 and len(props.Configs)==2
            assert str(props.OCGs[0].Name) in ['红色 Red','Renamed Red']
            assert props.D.ON[0].objgen == props.OCGs[0].objgen
            assert props.D.OFF[0].objgen == props.OCGs[1].objgen
            assert pdf.pages[0].Resources.XObject.Fm1.objgen == pdf.pages[1].Resources.XObject.Fm1.objgen
            assert '/OC' not in pdf.pages[0].Resources.XObject.Fm1
            # Each segment retains its source stream, rather than replacing page data.
            assert b'10 10 20 20 re f' in pdf.pages[0].Contents[0].read_bytes()
        clear=authored
        if encrypted:
            clear=OUT/(label+'-decrypted.pdf')
            run(label+'-decrypt','qpdf','--password=user','--decrypt',authored,clear)
        comparisons=0
        for config, selection in [('default','default'),('swap','0'),('both','1')]:
            pngdir=OUT/(label+'-'+config)
            run(label+'-'+config+'-skia',ROOT/'target/debug/examples/render_optional_content',clear,pngdir,selection,'view')
            for page in [1,2]:
                actual=Image.open(pngdir/f'page-{page}.png').convert('RGB')
                reference=Image.open(OUT/f'{config}-expected-{page}.png').convert('RGB')
                diff=ImageChops.difference(actual,reference)
                assert actual.size==reference.size==(200,200)
                assert max(v[1] for v in diff.getextrema())<=1,(label,config,page,diff.getbbox())
                assert ImageChops.difference(actual.point(lambda v:255 if v==255 else 0),reference.point(lambda v:255 if v==255 else 0)).getbbox() is None
                comparisons+=1
                pictures.append((label+' '+config+f' p{page}',pngdir/f'page-{page}.png'))
        # Independently render the authored default configuration itself.
        run(label+'-mupdf','mutool','draw','-q','-r','72','-o',OUT/(label+'-mupdf-%d.png'),clear)
        for page in [1,2]:
            a=Image.open(OUT/(label+f'-mupdf-{page}.png')).convert('RGB')
            b=Image.open(OUT/f'default-expected-{page}.png').convert('RGB')
            assert ImageChops.difference(a,b).getbbox() is None
        records.append(dict(case=label,sha256=hashlib.sha256(authored.read_bytes()).hexdigest(),comparisons=comparisons,mupdf_default=2))

for start in range(0,len(pictures),24):
    chunk=pictures[start:start+24]
    sheet=Image.new('RGB',(1000,6*235),'#dddddd');draw=ImageDraw.Draw(sheet)
    for index,(label,path) in enumerate(chunk):
        x=(index%4)*250;y=(index//4)*235
        sheet.paste(Image.open(path).convert('RGB'),(x+25,y+30));draw.text((x+3,y+3),label,fill='black')
    sheet.save(OUT/f'contact-{start//24+1}.png')
(OUT/'manifest.json').write_text(json.dumps(dict(license='AGPL-3.0-only',records=records,pixels=sum(r['comparisons'] for r in records),mupdf_default=sum(r['mupdf_default'] for r in records),versions={tool:run('version-'+tool,*cmd) for tool,cmd in [('qpdf',['qpdf','--version']),('mupdf',['mutool','-v'])]}),indent=2))
print(json.dumps(dict(cases=len(records),skia_pixel_pairs=len(pictures),mupdf_default_pairs=2*len(records))))
