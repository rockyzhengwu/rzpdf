#!/usr/bin/env python3
"""Create our own geometric font with a failed hint program and a PDF oracle.

Optional regeneration dependency: fontTools. Production and CI consume only the
small checked-in fixtures. No third-party font program is copied or modified.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import newTable
from fontTools.ttLib.tables.ttProgram import Program


def font_bytes(failed_hint):
    builder = FontBuilder(1000, isTTF=True)
    builder.setupGlyphOrder(['.notdef', 'space', 'A'])
    builder.setupCharacterMap({32: 'space', 65: 'A'})
    glyphs = {}
    for name in ['.notdef', 'space', 'A']:
        pen = TTGlyphPen(None)
        if name == 'A':
            pen.moveTo((50, 0)); pen.lineTo((550, 0)); pen.lineTo((550, 700))
            pen.lineTo((50, 700)); pen.closePath()
        glyphs[name] = pen.glyph()
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics({name: (600, 0) for name in glyphs})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(dict(familyName='RzHintRecovery', styleName='Regular',
                               uniqueFontIdentifier='RzHintRecovery1', fullName='RzHintRecovery',
                               psName='RzHintRecovery'))
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    builder.setupPost(); builder.setupMaxp()
    builder.font['head'].created = builder.font['head'].modified = 3876249600
    if failed_hint:
        prep = newTable('prep'); prep.program = Program()
        # A font hint CALL to an absent function: recover the valid outline.
        prep.program.fromBytecode([0xB0, 0x00, 0x2B])
        builder.font['prep'] = prep
    target = io.BytesIO(); builder.font.save(target, reorderTables=True)
    return target.getvalue()


def pdf(data):
    content = b'BT /F 1 Tf 10 0 0 10 10 30 Tm (AAAA AAAA) Tj ET'
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>',
               b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
               b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 80 50] /Resources << /Font << /F 4 0 R >> >> /Contents 7 0 R >>',
               b'<< /Type /Font /Subtype /TrueType /BaseFont /RzHintRecovery /Encoding /WinAnsiEncoding /FirstChar 32 /LastChar 65 /Widths [250 '+b'0 '*32+b'600] /FontDescriptor 5 0 R >>',
               b'<< /Type /FontDescriptor /FontName /RzHintRecovery /Flags 32 /FontBBox [0 0 600 800] /Ascent 800 /Descent -200 /CapHeight 700 /ItalicAngle 0 /StemV 80 /FontFile2 6 0 R >>',
               b'<< /Length '+str(len(data)).encode()+b' >>\nstream\n'+data+b'\nendstream',
               b'<< /Length '+str(len(content)).encode()+b' >>\nstream\n'+content+b'\nendstream']
    result = bytearray(b'%PDF-1.7\n%\xe2\xe3\xcf\xd3\n'); offsets=[0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(result)); result.extend(f'{index} 0 obj\n'.encode()+obj+b'\nendobj\n')
    xref=len(result); result.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]: result.extend(f'{offset:010} 00000 n \n'.encode())
    result.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    return bytes(result)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    for failed in [False, True]:
        (args.output/('hint-failed.pdf' if failed else 'hint-reference.pdf')).write_bytes(pdf(font_bytes(failed)))
    manifest={p.name:dict(bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(args.output.glob('*.pdf'))}
    (args.output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')


if __name__=='__main__':
    main()
