#!/usr/bin/env python3
"""Explicit, offline WA-T06/07/08 font fixtures; never downloads/discovers fonts.

Optional real CJK source is supplied by --cjk-collection and remains in ignored
output. FontTools is a fixture tool, not a runtime dependency.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
from fontTools import __version__ as fonttools_version
from fontTools.ttLib import TTFont, TTCollection
from fontTools import subset
from fontTools.cffLib import CFFFontSet
from prepare_webassembly_fixtures import ROOT, OUT, pdf, stream


def digest(data):
    return hashlib.sha256(data).hexdigest()


def source_pdf(text='AB', clip=False, language=None):
    points = list(map(ord, text))
    codes = bytes(range(65, 65 + len(points)))
    cmap = (b'1 begincodespacerange <00> <ff> endcodespacerange '
            + f'{len(points)} beginbfchar '.encode()
            + b' '.join(b'<' + bytes([code]).hex().encode() + b'> <'
                        + chr(point).encode('utf-16-be').hex().encode() + b'>'
                        for code, point in zip(codes, points)) + b' endbfchar')
    body = b'BT /F 20 Tf 1 0 0 1 20 30 Tm '
    if clip:
        body += b'7 Tr '
    body += b'<' + codes.hex().encode() + b'> Tj ET '
    if clip:
        body += b'0 0 1 rg 0 0 160 80 re f'
    objects = [('<< /Type /Catalog /Pages 2 0 R'+(' /Lang ('+language+')' if language else '')+' >>').encode(),
        b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 160 80] /Resources << /Font << /F 5 0 R >> >> /Contents 4 0 R >>',
        stream('', body),
        (f'<< /Type /Font /Subtype /TrueType /BaseFont /MissingMatrix /FirstChar 65 /LastChar {64+len(points)} '
         + '/Widths [' + ' '.join('900' if i==0 else ('1000' if len(points)>2 else '300') for i in range(len(points)))
         + '] /FontDescriptor << /Flags 0 >> /ToUnicode 6 0 R >>').encode(),
        stream('', cmap)]
    return pdf(objects)


def vertical_pdf(text):
    codes=[f'{i:04x}' for i in range(1,len(text)+1)]
    cmap=(f'1 begincodespacerange <0000> <ffff> endcodespacerange {len(text)} beginbfchar '
          +' '.join('<'+code+'> <'+ch.encode('utf-16-be').hex()+'>' for code,ch in zip(codes,text))+' endbfchar').encode()
    return pdf([b'<< /Type /Catalog /Pages 2 0 R /Lang (zh-Hans) >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 160 80] /Resources << /Font << /F 5 0 R >> >> /Contents 4 0 R >>',
        stream('',('BT /F 20 Tf 1 0 0 1 80 65 Tm <'+''.join(codes)+'> Tj ET').encode()),
        b'<< /Type /Font /Subtype /Type0 /BaseFont /MissingMatrix /Encoding /Identity-V /ToUnicode 6 0 R /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /MissingMatrix /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /W [1 [900 300 900]] /W2 [1 [-900 450 880 -1100 150 900 -1000 500 880]] /FontDescriptor << /Flags 0 >> >>] >>',stream('',cmap)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cjk-collection', type=Path)
    parser.add_argument('--cjk-license', type=Path)
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    records = []
    licenses = {'project': 'AGPL-3.0-only', 'urw': 'see font-data/NOTICE.md and upstream embedding exception', 'noto': 'OFL-1.1'}
    def asset(name, data):
        path = OUT / ('matrix-' + name)
        path.write_bytes(data)
        return {'file':path.name, 'sha256':digest(data), 'sizeBytes':len(data)}
    common = asset('missing.pdf', source_pdf())
    for name, path, fmt, lic in [
        ('truetype','pdf/tests/fixtures/v07/geometry.ttf','truetype','project'),
        ('opentype-cff','pdf/tests/fixtures/v07/geometry.otf','opentype','project'),
        ('raw-cff','pdf/tests/fixtures/v07/geometry.cff','cff','project'),
        ('type1','font-data/fonts/n022003l.pfb','type1','urw')]:
        records.append({'id':name,'input':common,'font':asset(name+'.'+path.rsplit('.',1)[1],(ROOT/path).read_bytes()),'format':fmt,'index':0,'unicode':'AB','advance':18,'license':licenses[lic]})
    one = TTFont(ROOT/'pdf/tests/fixtures/v07/geometry.ttf', recalcTimestamp=False)
    two = TTFont(ROOT/'pdf/tests/fixtures/v07/geometry.ttf', recalcTimestamp=False)
    two['head'].unitsPerEm = 2000
    collection = TTCollection(); collection.fonts = [one,two]
    buffer = io.BytesIO(); collection.save(buffer, shareTables=False)
    ttc = asset('two-faces.ttc', buffer.getvalue())
    for index in [0,1]:
        records.append({'id':'collection-'+str(index),'input':common,'font':ttc,'format':'collection','index':index,'unicode':'AB','advance':18,'license':licenses['project']})
    v07 = json.loads((ROOT/'pdf/tests/fixtures/v07/manifest.json').read_text())
    embedded = {'input':asset('embedded.pdf',(ROOT/'pdf/tests/fixtures/v07/fonts.pdf').read_bytes()),'cases':v07['cases']}
    # Sparse CID CFF: code/CID 42 selects physical GID 2. Explicit Unicode is B.
    cff_data = (ROOT/'pdf/tests/fixtures/synthetic-cid.cff').read_bytes()
    cff = CFFFontSet();cff.decompile(io.BytesIO(cff_data),None)
    assert cff.topDictIndex[0].charset[2]=='cid00042'
    cmap=b'1 begincodespacerange <0000> <ffff> endcodespacerange 1 beginbfchar <002a> <0042> endbfchar'
    cid_pdf=pdf([b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 40 40] /Resources << /Font << /F 5 0 R >> >> /Contents 4 0 R >>',
        stream('',b'BT /F 20 Tf 1 0 0 1 5 10 Tm <002a> Tj ET'),
        b'<< /Type /Font /Subtype /Type0 /BaseFont /RzpdfSyntheticCID /Encoding /Identity-H /ToUnicode 7 0 R /DescendantFonts [<< /Subtype /CIDFontType0 /BaseFont /RzpdfSyntheticCID /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor << /FontFile3 6 0 R >> >>] >>',
        stream('/Subtype /CIDFontType0C',cff_data),stream('',cmap)])
    sparse={'input':asset('sparse-cid.pdf',cid_pdf),'unicode':'B','cid':42,'gid':2}
    clip=pdf([b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 80 60] /Resources << /Font << /F 5 0 R >> >> /Contents 4 0 R >>',
        stream('',b'BT /F 20 Tf 7 Tr 10 20 Td (AB) Tj ET 0 0 1 rg 0 0 80 60 re f'),
        b'<< /Type /Font /Subtype /Type1 /BaseFont /RzpdfGeometry /Encoding /WinAnsiEncoding /FirstChar 65 /LastChar 66 /Widths [600 700] /FontDescriptor 6 0 R >>',
        b'<< /Type /FontDescriptor /FontName /RzpdfGeometry /Flags 32 /FontFile3 7 0 R >>',
        stream('/Subtype /Type1C',(ROOT/'pdf/tests/fixtures/v07/geometry.cff').read_bytes())])
    negative={'cffClip':asset('cff-clip.pdf',clip)}
    base_names=['Times-Roman','Times-Bold','Times-Italic','Times-BoldItalic','Helvetica','Helvetica-Bold',
        'Helvetica-Oblique','Helvetica-BoldOblique','Courier','Courier-Bold','Courier-Oblique','Courier-BoldOblique','Symbol','ZapfDingbats']
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',
        ('<< /Type /Pages /Count 14 /Kids ['+' '.join(f'{3+i*3} 0 R' for i in range(14))+'] >>').encode()]
    for i,name in enumerate(base_names):
        n=3+i*3
        objects.extend([(f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 160 80] /Resources << /Font << /F {n+2} 0 R >> >> /Contents {n+1} 0 R >>').encode(),
            stream('',b'BT /F 24 Tf 20 30 Td (AB) Tj ET'),
            f'<< /Type /Font /Subtype /Type1 /BaseFont /{name} /ToUnicode 45 0 R >>'.encode()])
    objects.append(stream('',b'1 begincodespacerange <00> <ff> endcodespacerange 2 beginbfchar <41> <0041> <42> <0042> endbfchar'))
    base14={'input':asset('base14.pdf',pdf(objects)),'names':base_names}
    cjk_source = None
    if args.cjk_collection:
        if not args.cjk_license:parser.error('--cjk-license is required with --cjk-collection')
        source = args.cjk_collection.read_bytes()
        notice = args.cjk_license.read_bytes()
        cjk_source={'sha256':digest(source),'sizeBytes':len(source),'license':asset('cjk-license.txt',notice)}
        # Frozen named-face selection, not implicit platform fallback.
        wanted={'Noto Sans CJK JP':('ja-JP','骨日本語'),'Noto Sans CJK KR':('ko-KR','骨한국어'),
                'Noto Sans CJK SC':('zh-Hans','骨汉字'),'Noto Sans CJK TC':('zh-Hant','骨漢字')}
        src=TTCollection(args.cjk_collection,lazy=True)
        found=set()
        for index,font in enumerate(src.fonts):
            family=font['name'].getDebugName(1)
            if family not in wanted:continue
            found.add(family);language,text=wanted[family]
            assert set(map(ord,text)).issubset(font.getBestCmap()),(family,text)
            options=subset.Options();options.name_IDs=['*'];options.name_languages=['*'];options.recalc_timestamp=False
            tool=subset.Subsetter(options=options);tool.populate(unicodes=map(ord,text));tool.subset(font)
            font.recalcTimestamp=False
            glyphs=[{'unicode':ord(ch),'gid':font.getGlyphID(font.getBestCmap()[ord(ch)])} for ch in text]
            buffer=io.BytesIO();font.save(buffer)
            id='cjk-'+language.lower()
            records.append({'id':id,'input':asset(id+'.pdf',source_pdf(text,language=language)),
                'font':asset(id+'.otf',buffer.getvalue()),'format':'opentype','index':0,'sourceIndex':index,
                'unicode':text,'advance':18,'language':language,'license':'OFL-1.1','fonttoolsGlyphs':glyphs,
                'sourceFamily':family})
            if language=='zh-Hans':
                vertical=dict(records[-1]);vertical.update(id=id+'-vertical',input=asset(id+'-vertical.pdf',vertical_pdf(text)),writingMode='Vertical')
                records.append(vertical)
        assert found==set(wanted),('required named CJK faces missing',found)
        src.close()
    manifest={'version':1,'fonttools':fonttools_version,'records':records,'embedded':embedded,'sparseCid':sparse,'negative':negative,'base14':base14,
        'cjkSource':cjk_source,'cjkMatrix':'included' if cjk_source else 'not supplied; real CJK matrix unverified',
        'sourceLicenses':licenses,'notes':['No host font discovery, download or font program rewrite in runtime.',
            'Collection face 1 has deliberately different em; both faces cover AB.',
            'Source PDF widths differ from font widths; rendering must retain PDF positioning.']}
    (OUT/'font-matrix.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'records':len(records),'embeddedPages':len(v07['cases']),'cjk':manifest['cjkMatrix']}))

if __name__=='__main__':main()
