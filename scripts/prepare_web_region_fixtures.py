#!/usr/bin/env python3
"""Project-authored region-rendering probes. No downloads or private inputs."""
import hashlib
import json
from pathlib import Path
from prepare_webassembly_fixtures import OUT, pdf, stream


def fixture(resources, content, extra=(), size=(40,30), page_extra=''):
    return pdf([b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        (f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {size[0]} {size[1]}] /Resources << '+resources+' >> /Contents 4 0 R '+page_extra+' >>').encode(),
        stream('',content.encode()),*extra])


def main():
    OUT.mkdir(parents=True,exist_ok=True);records=[]
    def add(id,data,**extra):
        p=OUT/('region-'+id+'.pdf');p.write_bytes(data)
        records.append({'id':id,'file':p.name,'sha256':hashlib.sha256(data).hexdigest(),**extra})
    add('fractional-edges',fixture('', '0 1 0 rg 0 0 30 20 re f 1 1 0 rg 25 0 5 20 re f',size=(30,20)))
    add('glyph-phase',fixture('/Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>', 'BT /F 200 Tf 1 0 0 1 0.124999 30 Tm (AOS) Tj ET',size=(480,260)),curveRaster=True,stressMaxWasmBytes=128*1024*1024)
    add('glyph-phase-pattern',fixture('/Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> /Pattern << /P 5 0 R >>', '/Pattern cs /P scn BT /F 200 Tf 1 0 0 1 0.124999 30 Tm (AOS) Tj ET',extra=(stream('/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 8 8] /XStep 8 /YStep 8 /Resources << >>',b'1 0 0 rg 0 0 4 8 re f 0 0 1 rg 4 0 4 8 re f'),),size=(480,260)),curveRaster=True,stressMaxWasmBytes=128*1024*1024)
    add('curve-clip',fixture('', 'q 1 0.2 0.1 1 0.3 0.7 cm 10 50 m 10 5 120 5 120 50 c 120 100 10 100 10 50 c W n 0 0 1 rg 0 0 150 120 re f Q',size=(160,140)),curveRaster=True)
    add('polygon-clip',fixture('', 'q 1 0.2 0.1 1 0.3 0.7 cm 10.2 50.7 m 50.3 10.2 l 125.7 50.3 l 70.8 110.1 l h W n 0 0 1 rg 0 0 150 120 re f Q',size=(160,140)),curveRaster=True)
    add('skew-rect-clip',fixture('', 'q 1 0.2 0.1 1 0.3 0.7 cm 12.2 10.7 100.5 80.4 re W n 0 0 1 rg 0 0 150 120 re f Q',size=(160,140)),curveRaster=True)
    add('polygon-fill',fixture('', 'q 1 0.2 0.1 1 0.3 0.7 cm 0 0 1 rg 10.2 50.7 m 50.3 10.2 l 125.7 50.3 l 70.8 110.1 l h f Q',size=(160,140)),curveRaster=True)
    add('skew-rect-fill',fixture('', 'q 1 0.2 0.1 1 0.3 0.7 cm 0 0 1 rg 12.2 10.7 100.5 80.4 re f Q',size=(160,140)),curveRaster=True)
    geometry_font = (Path(__file__).resolve().parents[1]/'pdf/tests/fixtures/v07/geometry.ttf').read_bytes()
    add('text-clip',fixture('/Font << /F 5 0 R >>', 'q 1 0.2 0.1 1 0.3 0.7 cm BT /F 64 Tf 7 Tr 1 0 0 1 16 20 Tm (A) Tj 1 0 0 1 70 35 Tm (A) Tj ET 1 0 0 rg 0 0 150 120 re f Q',extra=(b'<< /Type /Font /Subtype /TrueType /BaseFont /Geometry /FirstChar 65 /LastChar 65 /Widths [600] /Encoding /WinAnsiEncoding /FontDescriptor 6 0 R >>',b'<< /Type /FontDescriptor /FontName /Geometry /Flags 32 /FontBBox [0 -200 1000 1000] /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>',stream('/Length1 '+str(len(geometry_font)),geometry_font)),size=(160,140)),curveRaster=True)
    clip = '1 0.2 0.1 1 0.3 0.7 cm 10 50 m 10 5 120 5 120 50 c 120 100 10 100 10 50 c W n '
    add('curve-clip-nested',fixture('', 'q '+clip+'q 22 30 m 110 20 130 60 100 90 c 20 120 10 70 22 30 c h 50 45 35 25 re W* n 0 0 1 rg 0 0 150 120 re f Q Q',size=(160,140)),curveRaster=True)
    add('curve-clip-alpha',fixture('/ExtGState << /A << /ca 0.5 /BM /Multiply >> >>', '0 1 0 rg 0 0 160 140 re f q '+clip+'/A gs 0 0 1 rg 0 0 150 120 re f Q',size=(160,140)),curveRaster=True,pixelOracle=[{'x':65,'y':70,'rgba':[0,127,0,255]}])
    add('curve-clip-pattern',fixture('/Pattern << /P 5 0 R >>', 'q '+clip+'/Pattern cs /P scn 0 0 150 120 re f Q',extra=(stream('/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 8 8] /XStep 8 /YStep 8 /Resources << >>',b'1 0 0 rg 0 0 4 8 re f 0 0 1 rg 4 0 4 8 re f'),),size=(160,140)),curveRaster=True)
    form=stream('/Subtype /Form /BBox [0 0 20 20] /Resources << /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >> /Group << /S /Transparency /I true /K false /CS /DeviceRGB >>',
        b'1 0 0 rg 0 0 20 20 re f 0 0 1 rg 10 0 10 20 re f BT /F 8 Tf 2 4 Td (A) Tj ET')
    add('stroke-text',fixture('/Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>', 'q 1 0.2 0.1 1 0.3 0.7 cm 0.5 w BT /F 14 Tf 1 Tr 1 0 0 1 25.3 22.7 Tm (OS) Tj ET 0 0 1 RG BT /F 14 Tf 2 Tr 1 0 0 1 70.3 22.7 Tm (OS) Tj ET Q',size=(160,80)),curveRaster=True)
    add('hairline-curve',fixture('', 'q 1 0.2 0.1 1 0.3 0.7 cm 0.01 w [2 1] 0 d 3 3 m 3 25 35 25 35 3 c S Q'),curveRaster=True)
    add('group',fixture('/ExtGState << /Half << /ca 0.5 >> >> /XObject << /G 5 0 R >>',
        '0 1 0 rg 0 0 30 20 re f q /Half gs /G Do Q 1 1 0 rg 25 0 5 20 re f',[form],size=(30,20)))
    form=stream('/Subtype /Form /BBox [0 0 40 30] /Resources << /ExtGState << /Half << /ca 0.5 >> /Zero << /ca 0 >> >> >> /Group << /S /Transparency /I false /K true /CS /DeviceRGB >>',
        b'1 0 0 rg 0 0 30 30 re f q /Half gs 0 0 1 rg 10 0 10 30 re f Q /Zero gs 20 0 10 30 re f')
    add('knockout',fixture('/XObject << /G 5 0 R >>','0 1 0 rg 0 0 40 30 re f /G Do',[form]))
    mask=stream('/Subtype /Form /BBox [0 0 40 30] /Resources << >> /Group << /S /Transparency /I true /CS /DeviceGray >>',b'0.5 g 0 0 20 30 re f 0.8 g 20 0 20 30 re f')
    add('soft-mask',fixture('/ExtGState << /M << /SMask << /S /Luminosity /G 5 0 R /BC [0] >> >> >>',
        '/M gs 0 0 1 rg 0 0 40 30 re f',[mask]))
    pattern=stream('/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 4 4] /XStep 10 /YStep 10 /Matrix [1 0.2 0.1 1 0.3 0.7] /Resources << >>',b'1 0 0 rg 0 0 4 4 re f')
    add('pattern',fixture('/Pattern << /P 5 0 R >>','/Pattern cs /P scn 0 0 40 30 re f',[pattern]))
    pattern=stream('/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << /ExtGState << /M << /BM /Multiply >> >> >>',b'/M gs 0.5 g 0 0 10 10 re f')
    add('pattern-backdrop-rounded',fixture('/Pattern << /P 5 0 R >>','0 1 0 rg 0 0 20 20 re f /Pattern cs /P scn 0 0 20 20 re f',[pattern],size=(20,20)))
    shading=b'<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 40 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> /Extend [true true] >>'
    add('masked-shading',fixture('/ExtGState << /M << /SMask << /S /Luminosity /G 5 0 R /BC [0] >> >> >> /Shading << /S 6 0 R >>','/M gs /S sh',[mask,shading]))
    add('geometry',fixture('/Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >>',
        '0 1 0 rg 5 3 40 30 re f 0 0 1 rg 10 8 20 10 re f BT /F 6 Tf 12 22 Td (A B) Tj ET',size=(50,40),page_extra='/CropBox [5 3 45 33] /Rotate 90 /UserUnit 1.5'))
    for id,file in [('folded-patch','folded-patch.pdf'),('image-mask','fine-soft-mask.pdf'),('codecs','composite.pdf')]:
        add(id,(OUT/file).read_bytes())
    add('widget-flags',(OUT/'interaction-flags-28.pdf').read_bytes(),tileSize=128,curveRaster=True,nativeProfile={'differingChannels':1050,'maxChannelDifference':170,'bounds':[197,19,218,60],'reason':'Reviewed existing embedded Noto Chinese edge difference at rotated UserUnit 1.5; all outside pixels exact'})
    add('widget-flags-zoom',(OUT/'interaction-flags-28.pdf').read_bytes(),tileSize=128,curveRaster=True,options={'viewZoom':2},nativeFull=False)
    widget=pdf([b'<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] /DA (/F1 12 Tf 0 g) /DR << /Font << /F1 7 0 R >> >> >> >>',
        b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 120 100] /CropBox [5 3 115 93] /Rotate 90 /UserUnit 1.5 /Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R /Annots [5 0 R] >>',
        stream('',b'0.9 1 0.9 rg 5 3 110 90 re f'),
        b'<< /Type /Annot /Subtype /Widget /FT /Tx /T (region-field) /V (Form) /DA (/F1 12 Tf 0 g) /Rect [20 20 100 50] /F 28 /P 3 0 R /AP << /N 6 0 R >> >>',
        stream('/Subtype /Form /BBox [0 0 80 30] /Resources << /Font << /F1 7 0 R >> >>',b'0.8 0.9 1 rg 0 0 80 30 re f 0 0 1 RG 1 w 1 1 78 28 re S 0 g BT /F1 12 Tf 4 10 Td (Form) Tj ET'),
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'])
    add('widget-base14',widget,tileSize=128)
    add('widget-base14-zoom',widget,tileSize=128,options={'viewZoom':2},nativeFull=False)
    add('large',fixture('', '1 0 0 rg 0 0 6000 4000 re f 0 1 0 rg 5990 3990 10 10 re f',size=(6000,4000)),large=True)
    (OUT/'region-matrix.json').write_text(json.dumps({'version':1,'license':'Project graphics: AGPL-3.0-only; bundled Base14 and embedded Noto subsets retain upstream licenses','notices':['font-data/NOTICE.md','docs/licenses/NotoSansSC-OFL.txt'],'records':records},indent=2)+'\n')
    print(json.dumps({'records':len(records)}))

if __name__=='__main__':main()
