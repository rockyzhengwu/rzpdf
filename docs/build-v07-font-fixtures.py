"""Generate the project-authored v0.7 fixed font matrix with FontTools/pikepdf.

Usage: PYTHONPATH=tmp/pdfs/v05-verifiers-py311 python3.11 docs/build-v07-font-fixtures.py
No network, installed fonts, timestamps or PDF-engine output as an oracle.
Geometric test fonts/programs are project-authored under the repository license.
The embedded Courier PFB is an unchanged existing licensed font-data asset.
"""
import hashlib
import io
import json
from pathlib import Path
import fontTools
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
import pikepdf as p

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "pdf/tests/fixtures/v07"
OUT.mkdir(parents=True, exist_ok=True)
ORDER = [".notdef", "A", "B"]
SHAPES = {".notdef": [], "A": [(0, 0), (500, 0), (500, 700), (0, 700)], "B": [(0, 0), (600, 0), (300, 700)]}
WIDTHS = {".notdef": (600, 0), "A": (600, 0), "B": (700, 0)}


def draw(pen, points):
    if points:
        pen.moveTo(points[0])
        for point in points[1:]: pen.lineTo(point)
        pen.closePath()


def font(cff):
    fb = FontBuilder(1000, isTTF=not cff)
    fb.setupGlyphOrder(ORDER)
    fb.setupCharacterMap({65: "A", 66: "B"})
    fb.setupHorizontalMetrics(WIDTHS)
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({"familyName": "RzpdfV07Geometry", "styleName": "Regular", "psName": "RzpdfV07Geometry", "uniqueFontIdentifier": "RzpdfV07Geometry", "fullName": "RzpdfV07Geometry"})
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    fb.setupPost()
    if cff:
        programs = {}
        for name in ORDER:
            pen = T2CharStringPen(WIDTHS[name][0], None)
            draw(pen, SHAPES[name])
            programs[name] = pen.getCharString()
        fb.setupCFF("RzpdfV07Geometry", {"FullName": "RzpdfV07Geometry", "FamilyName": "RzpdfV07Geometry", "Weight": "Regular", "FontBBox": [0, 0, 600, 700]}, programs, {})
    else:
        glyphs = {}
        for name in ORDER:
            pen = TTGlyphPen(None)
            draw(pen, SHAPES[name])
            glyphs[name] = pen.glyph()
        fb.setupGlyf(glyphs)
        fb.setupVerticalMetrics({name: (1000, 100) for name in ORDER})
        fb.setupVerticalHeader(ascent=800, descent=-200)
    fb.font["head"].created = fb.font["head"].modified = 2082844800
    fb.font.recalcTimestamp = False
    output = io.BytesIO()
    fb.save(output)
    return output.getvalue(), fb.font


ttf, _ = font(False)
otf, otfont = font(True)
cff = otfont["CFF "].compile(otfont)
for name, data in [("geometry.ttf", ttf), ("geometry.otf", otf), ("geometry.cff", cff)]:
    (OUT / name).write_bytes(data)

CASES = ["cff", "opentype-cff", "simple-truetype", "cid-h", "cid-v", "cid-lengths", "embedded-type1", "differences"]
manifest = {"generator": {"fonttools": fontTools.__version__, "pikepdf": p.__version__}, "cases": []}
with p.Pdf.new() as pdf:
    for kind in CASES:
        page = pdf.add_blank_page(page_size=(180, 180))
        if kind == "simple-truetype": page.Rotate = 90
        data = ttf if kind in ("simple-truetype", "cid-h", "cid-v", "cid-lengths") else otf if kind == "opentype-cff" else cff
        program = pdf.make_stream(data)
        descriptor = p.Dictionary(Type=p.Name.FontDescriptor, FontName=p.Name.RzpdfV07Geometry, Flags=32, FontBBox=p.Array([0, 0, 600, 700]), ItalicAngle=0, Ascent=800, Descent=-200, CapHeight=700, StemV=80)
        if kind in ("simple-truetype", "cid-h", "cid-v", "cid-lengths"):
            descriptor.FontFile2 = program
        else:
            program.Subtype = p.Name.OpenType if kind == "opentype-cff" else p.Name.Type1C
            descriptor.FontFile3 = program
        codes = [b"A", b"B"]
        texts = ["fi", "😀"]
        gids = [1, 2]
        widths = [600, 700]
        origins = [[0, 0], [0, 0]]
        if kind.startswith("cid-"):
            vertical = kind == "cid-v"
            codes = [b"A", b"\x80\x01"] if kind != "cid-lengths" else [b"A", b"\x80\x01", b"\x81\0\x01", b"\x82\0\0\x01"]
            if kind == "cid-lengths":
                texts, gids, widths = ["fi", "😀", "中", "AB"], [1, 2, 1, 2], [600, 700, 600, 700]
            cids = [7, 19] if len(codes) == 2 else [7, 19, 21, 23]
            spaces = "<40> <7F> <8000> <80FF>" if kind != "cid-lengths" else "<40> <7F> <8000> <80FF> <810000> <81FFFF> <82000000> <82FFFFFF>"
            encoding = f"/WMode {int(vertical)} def {len(codes)} begincodespacerange {spaces} endcodespacerange {len(codes)} begincidchar "
            encoding += " ".join(f"<{code.hex()}> {cid}" for code, cid in zip(codes, cids)) + " endcidchar"
            mapping = bytearray(48)
            for cid, gid in zip(cids, gids): mapping[cid*2:cid*2+2] = gid.to_bytes(2, "big")
            w = p.Array()
            for cid, width in zip(cids, widths): w.extend([cid, p.Array([width])])
            descendant = pdf.make_indirect(p.Dictionary(Type=p.Name.Font, Subtype=p.Name.CIDFontType2, BaseFont=p.Name.RzpdfV07Geometry, CIDSystemInfo=p.Dictionary(Registry=p.String("Adobe"), Ordering=p.String("Identity"), Supplement=0), FontDescriptor=pdf.make_indirect(descriptor), DW=500, W=w, CIDToGIDMap=pdf.make_stream(bytes(mapping))))
            if vertical:
                descendant.W2 = p.Array([7, p.Array([-900, 300, 800]), 19, p.Array([-1100, 350, 850])])
                widths, origins = [-900, -1100], [[300, 800], [350, 850]]
            else: origins = [[0, 0]] * len(codes)
            f = p.Dictionary(Type=p.Name.Font, Subtype=p.Name.Type0, BaseFont=p.Name.RzpdfV07Geometry, Encoding=pdf.make_stream(encoding.encode()), DescendantFonts=p.Array([descendant]))
        elif kind in ("embedded-type1", "differences"):
            codes, texts = [b"A", b"B"], ["A", "B"]
            f = p.Dictionary(Type=p.Name.Font, Subtype=p.Name.Type1, BaseFont=p.Name.Courier, Encoding=p.Name.WinAnsiEncoding)
            widths = [600, 600]
            # Independent FreeType/MuPDF comparisons below establish actual
            # Courier GIDs rather than pretending its glyph order is 1/2.
            gids = None
            if kind == "embedded-type1":
                pfb = (ROOT / "font-data/fonts/n022003l.pfb").read_bytes()
                segments, offset = [], 0
                while pfb[offset:offset+2] != b"\x80\x03":
                    assert pfb[offset] == 128
                    length = int.from_bytes(pfb[offset+2:offset+6], "little")
                    segments.append((pfb[offset+1], pfb[offset+6:offset+6+length]))
                    offset += length + 6
                raw = b"".join(data for _, data in segments)
                program = pdf.make_stream(raw)
                program.Length1, program.Length2, program.Length3 = [len(data) for _, data in segments]
                descriptor = p.Dictionary(Type=p.Name.FontDescriptor, FontName=p.Name.Courier, Flags=33, FontBBox=p.Array([-50, -200, 1000, 900]), ItalicAngle=0, Ascent=800, Descent=-200, CapHeight=700, StemV=80, FontFile=program)
                f.FontDescriptor = pdf.make_indirect(descriptor)
                f.FirstChar, f.LastChar, f.Widths = 65, 66, p.Array([600, 600])
            else:
                f.Encoding = p.Dictionary(BaseEncoding=p.Name.WinAnsiEncoding, Differences=p.Array([65, p.Name.B, p.Name.A]))
                # Drawing BA; extraction deliberately remains A/B.
        else:
            f = p.Dictionary(Type=p.Name.Font, Subtype=p.Name.TrueType if kind == "simple-truetype" else p.Name.Type1, BaseFont=p.Name.RzpdfV07Geometry, Encoding=p.Name.WinAnsiEncoding, FirstChar=65, LastChar=66, Widths=p.Array([600, 700]), FontDescriptor=pdf.make_indirect(descriptor))
        unicode = f"{len(codes)} beginbfchar " + " ".join(f"<{code.hex()}> <{text.encode('utf-16-be').hex()}>" for code, text in zip(codes, texts)) + " endbfchar"
        f.ToUnicode = pdf.make_stream(unicode.encode())
        page.Resources = p.Dictionary(Font=p.Dictionary(F0=pdf.make_indirect(f)))
        body = "BT /F0 20 Tf 1 0 0 1 30 130 Tm <" + b"".join(codes).hex() + "> Tj ET"
        page.Contents = pdf.make_stream(body.encode())
        manifest["cases"].append(dict(kind=kind, codes=[c.hex().upper() for c in codes], unicode=texts, gids=gids, widths=widths, origins=origins, rotation=int(page.get("/Rotate", 0))))
    pdf.save(OUT / "fonts.pdf", deterministic_id=True, min_version="1.7")
manifest["sha256"] = {name: hashlib.sha256((OUT / name).read_bytes()).hexdigest() for name in ["fonts.pdf", "geometry.ttf", "geometry.otf", "geometry.cff"]}
manifest["pfb_source_sha256"] = hashlib.sha256((ROOT / "font-data/fonts/n022003l.pfb").read_bytes()).hexdigest()
(OUT / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
print(json.dumps(manifest, ensure_ascii=False))
