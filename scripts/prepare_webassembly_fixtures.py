#!/usr/bin/env python3
"""WA-03 synthetic JPEG/mask/alpha fixture; no corpus download or private PDF."""
import hashlib
import io
import json
import struct
import subprocess
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "output/webassembly/fixtures"

def pdf(objects):
    data = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{i} 0 obj\n".encode() + obj + b"\nendobj\n")
    start = len(data)
    data.extend(f"xref\n0 {len(objects)+1}\n0000000000 65535 f \n".encode())
    for offset in offsets:
        data.extend(f"{offset:010} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n".encode())
    return bytes(data)

def stream(properties, data):
    return f"<< {properties} /Length {len(data)} >>\nstream\n".encode() + data + b"\nendstream"

if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    image = Image.new("RGB", (8, 8))
    image.putdata([(x*32, y*32, 128) for y in range(8) for x in range(8)])
    jpeg = io.BytesIO(); image.save(jpeg, "JPEG", quality=95, subsampling=0)
    content = b"1 1 1 rg 0 0 96 96 re f 1 0 0 rg 8 8 40 40 re f q /GS gs 0 0 1 rg 28 28 40 40 re f Q q 24 0 0 24 68 68 cm /I Do Q q 24 0 0 24 68 8 cm /M Do Q 0 0 0 rg BT /F1 16 Tf 8 76 Td (Wasm) Tj ET"
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Resources << /Font << /F1 4 0 R >> /ExtGState << /GS 5 0 R >> /XObject << /I 7 0 R /M 8 0 R >> >> /Contents 6 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", b"<< /Type /ExtGState /Name /GS1 /ca 0.5 >>", stream("", content),
        stream("/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode", jpeg.getvalue()),
        stream("/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /BitsPerComponent 1", bytes([0xAA,0x55]*4))]
    generated = OUT / "composite.pdf"; generated.write_bytes(pdf(objects))
    cases = {"composite": generated, "embedded-fonts": ROOT/"pdf/tests/fixtures/v07/fonts.pdf", "codecs": ROOT/"pdf/tests/fixtures/v06/external-codecs.pdf", "r4-empty": ROOT/"pdf/tests/fixtures/r4/empty-user.pdf"}
    # A valid empty symbol dictionary routes the checkerboard through jbig2dec,
    # exercising its Wasm allocator callbacks without borrowing scanned content.
    encoded = (ROOT/"pdf/tests/fixtures/v06/generic-mmr.jb2").read_bytes()
    segments = []; offset = 0
    while offset < len(encoded):
        number, flags, references, association, length = struct.unpack_from(">IBBBI", encoded, offset)
        if references: raise ValueError("expected standalone generic fixture")
        offset += 11; body = encoded[offset:offset+length]; offset += length
        segments.append((flags,association,body))
    def segment(number, flags, association, body):
        return struct.pack(">IBBBI",number,flags,0,association,len(body))+body
    native = segment(1,*segments[0])
    native += segment(2,0,1,struct.pack(">HII",1,0,0)+bytes(8))
    native += segment(3,*segments[1])+segment(4,*segments[2])
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /XObject << /I 5 0 R >> >> /Contents 4 0 R >>",
        stream("",b"q 32 0 0 32 0 0 cm /I Do Q"),
        stream("/Type /XObject /Subtype /Image /Width 32 /Height 32 /BitsPerComponent 1 /ColorSpace /DeviceGray /Filter /JBIG2Decode",native)]
    path = OUT/"native-route.pdf";path.write_bytes(pdf(objects));cases["native-route"]=path
    # Project-authored regressions distilled from corpus root causes.
    shared = b"/Font << /F 4 0 R >> /ExtGState << /S << /BM /Screen >> /L << /BM /SoftLight >> >> /XObject << /Fine 5 0 R /Jpx 7 0 R /Nearest 8 0 R >>"
    bodies = [b"q /S gs 0 1 1 rg 4 4 8 8 re f Q q /L gs 0 0 0 rg 20 20 8 8 re f Q",
              b"q 32 0 0 32 0 0 cm /Fine Do Q",
              b"BT /F 1 Tf 1 Tr 12 0 0 12 8 8 Tm (H) Tj ET",
              b"q 32 0 0 32 0 0 cm /Jpx Do Q",
              b"q 4 0 0 1 0 0 cm /Nearest Do Q",
              b"BT /F 10 Tf 1 0 0 1 2 2 Tm (A\\r\\nB) Tj ET 0.001 w 2 20 m 30 20 l S"]
    kids = b" ".join(f"{9+i*2} 0 R".encode() for i in range(len(bodies)))
    def jp2box(tag, body):
        return struct.pack(">I",len(body)+8)+tag+body
    gray=(ROOT/"pdf/tests/fixtures/v06/gray-lossless.jp2").read_bytes()
    palette=bytes([1,0,3,7,7,7])+b"".join(bytes([v,255-v,0]) for v in range(256))
    mapping=bytes([0,0,1,0,0,0,1,1,0,0,1,2])
    encoded=bytearray();offset=0
    while offset<len(gray):
        length=struct.unpack_from(">I",gray,offset)[0];tag=gray[offset+4:offset+8]
        body=gray[offset+8:offset+length]
        if tag==b"jp2h":
            body=bytearray(body);color=body.index(b"colr");body[color+7:color+11]=struct.pack(">I",16)
            body+=jp2box(b"pclr",palette)+jp2box(b"cmap",mapping)
            encoded+=jp2box(tag,body)
        else:encoded+=gray[offset:offset+length]
        offset+=length
    encoded=encoded.hex().encode()+b">"
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Kids ["+kids+b"] /Count 6 >>",b"null",
               b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
               stream("/Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Mask 6 0 R",bytes([0,0,255])),
               stream("/Subtype /Image /Width 32 /Height 32 /ImageMask true /BitsPerComponent 1",bytes([0xAA,0x55,0xAA,0x55])*32),
               stream("/Subtype /Image /Width 16 /Height 16 /Filter [/ASCIIHexDecode /JPXDecode]",encoded),
               stream("/Subtype /Image /Width 8 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate false",bytes([255,0,0,0,0,255])*4)]
    for i,body in enumerate(bodies):
        objects.extend([b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << "+shared+b" >> /Contents "+f"{10+i*2} 0 R >>".encode(),stream("",body)])
    path=OUT/"compat-regressions.pdf";path.write_bytes(pdf(objects));cases["compat-regressions"]=path
    # Independent, project-authored straight-opacity and CMYK regression pages.
    channels = [("gray-alpha.jp2", 1), ("rgb-alpha.jp2", 1),
                ("rgb-alpha.jp2", 0), ("cmyk-lossless.jp2", 0)]
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",
               b"<< /Type /Pages /Count 4 /Kids [3 0 R 6 0 R 9 0 R 12 0 R] >>"]
    for index, (name, alpha) in enumerate(channels):
        number = 3 + index * 3
        objects.extend([
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /XObject << /I {number+2} 0 R >> >> /Contents {number+1} 0 R >>".encode(),
            stream("", b"0 1 0 rg 0 0 32 32 re f q 32 0 0 32 0 0 cm /I Do Q"),
            stream(f"/Subtype /Image /Width 4 /Height 4 /Filter /JPXDecode /SMaskInData {alpha}",
                   (ROOT/"pdf/tests/fixtures/v06"/name).read_bytes())])
    path = OUT/"jpx-channels.pdf"
    path.write_bytes(pdf(objects))
    cases["jpx-channels"] = path
    # OpenJPEG independently encodes an oversized nominal tile (not header surgery).
    image = Image.new("RGB", (4, 4))
    image.putdata([(x*85, y*85, 64) for y in range(4) for x in range(4)])
    buffer = io.BytesIO()
    image.save(buffer, "JPEG2000", tile_size=(128, 128), num_resolutions=3,
               irreversible=False, no_jp2=True)
    # Redundant COC changes no coding settings; validate the result with OpenJPEG.
    encoded = buffer.getvalue()
    cod = encoded.index(b"\xff\x52")
    length = int.from_bytes(encoded[cod+2:cod+4], "big")
    payload = encoded[cod+4:cod+2+length]
    body = bytes([2, payload[0]]) + payload[5:]
    coc = b"\xff\x53" + (len(body)+2).to_bytes(2, "big") + body
    encoded = encoded[:cod+2+length] + coc + encoded[cod+2+length:]
    if Image.open(io.BytesIO(encoded)).tobytes() != image.tobytes():
        raise ValueError("independent OpenJPEG source pixels differ")
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",
               b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /XObject << /I 5 0 R >> >> /Contents 4 0 R >>",
               stream("", b"q 32 0 0 32 0 0 cm /I Do Q"),
               stream("/Subtype /Image /Width 4 /Height 4 /Filter /JPXDecode", encoded)]
    path = OUT/"jpx-large-tile.pdf"
    path.write_bytes(pdf(objects))
    cases["jpx-large-tile"] = path
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /XObject << /I 5 0 R >> >> /Contents 4 0 R >>",
               stream("",b"0 1 0 rg 0 0 32 32 re f q 32 0 0 32 0 0 cm /I Do Q"),
               stream("/Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /SMask 6 0 R",bytes([255,0,0])),
               stream("/Subtype /Image /Width 16 /Height 16 /BitsPerComponent 1 /ColorSpace /DeviceGray",bytes([0xAA,0xAA,0x55,0x55])*8)]
    path = OUT/"fine-soft-mask.pdf"
    path.write_bytes(pdf(objects))
    cases["fine-soft-mask"] = path
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /XObject << /I 5 0 R >> >> /Contents 4 0 R >>",
               stream("", b"q 32 0 0 32 0 0 cm /I Do Q"),
               stream("/Subtype /Image /Width 4 /Height 4 /Filter /JPXDecode /ColorSpace [/Indexed /DeviceRGB 3 <FF000000FF000000FFFFFFFF>]",
                      (ROOT/"pdf/tests/fixtures/v06/gray-indices-2bit.jp2").read_bytes())]
    path = OUT/"jpx-pdf-indices.pdf"
    path.write_bytes(pdf(objects))
    cases["jpx-pdf-indices"] = path
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /Font << /F 5 0 R >> /ExtGState << /S << /ca 0.5 /CA 0.5 >> >> >> /Contents 4 0 R >>",
               stream("",b"1 1 1 rg 0 0 32 32 re f /S gs BT /F 1 Tf (A) Tj ET"),
               b"<< /Type /Font /Subtype /Type3 /FontMatrix [1 0 0 1 0 0] /FontBBox [0 0 32 32] /Encoding << /Differences [65 /A] >> /FirstChar 65 /LastChar 65 /Widths [20] /CharProcs << /A 6 0 R >> >>",
               stream("",b"20 0 d0 1 0 0 rg 2 2 16 16 re f 10 2 16 16 re f")]
    path=OUT/"type3-alpha.pdf"
    path.write_bytes(pdf(objects))
    cases["type3-alpha"]=path
    mmr = segments[1][2][18:]
    content = b"q 32 0 0 32 0 0 cm BI /W 32 /H 32 /BPC 1 /CS /G /F /CCF /DP << /K -1 /Columns 32 >> ID " + mmr + b"\nEI Q"
    objects=[b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
             b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << >> /Contents 4 0 R >>",stream("",content)]
    path=OUT/"inline-ccitt.pdf";path.write_bytes(pdf(objects));cases["inline-ccitt"]=path
    objects=[b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
             b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /ExtGState << /M 5 0 R >> >> /Contents 4 0 R >>",
             stream("",b"/M gs 0 0 1 rg 0 0 32 32 re f"),
             b"<< /SMask << /S /Luminosity /G 6 0 R >> >>",
             stream("/Subtype /Form /BBox [0 0 32 32] /Resources << >> /Group << /S /Transparency /I true /CS /DeviceGray >>",b"0.5 g 0 0 16 32 re f")]
    path=OUT/"gray-mask-group.pdf";path.write_bytes(pdf(objects));cases["gray-mask-group"]=path
    objects=[b"<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R] /D << /BaseState /OFF >> >> >>",
             b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
             b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /Properties << /Layer 5 0 R >> >> /Contents 4 0 R >>",
             stream("",b"/OC /Layer BDC 1 0 0 rg 0 0 16 32 re f EMC 0 1 0 rg 16 0 16 32 re f"),
             b"<< /Type /OCG /Name (Hidden) >>"]
    path=OUT/"default-hidden-layer.pdf";path.write_bytes(pdf(objects));cases["default-hidden-layer"]=path
    points=[(255,0),(255,85),(255,170),(255,255),(0,255),(0,255),
            (255,255),(255,170),(255,85),(255,0),(0,0),(0,0)]
    encoded=bytes([0])+bytes(v for point in points for v in point)+bytes([0,0,255,255])
    objects=[b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
             b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /Shading << /S 5 0 R >> >> /Contents 4 0 R >>",
             stream("",b"q 10 0 0 10 0.5 1 cm /S sh Q"),
             stream("/ShadingType 6 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [-1 3 0 3 0 1]",encoded)]
    path=OUT/"folded-patch.pdf";path.write_bytes(pdf(objects));cases["folded-patch"]=path
    # WA-WEB-01/02/03: a self-authored two-page navigation and visible-text fixture.
    objects=[b"<< /Type /Catalog /Pages 2 0 R /Outlines 9 0 R /OCProperties << /OCGs [8 0 R] /D << /OFF [8 0 R] >> >> >>",
        b"<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 160 100] /Resources << /Font << /F 7 0 R >> /Properties << /Hidden 8 0 R >> >> /Contents 5 0 R /Annots [11 0 R 12 0 R 13 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 160 100] /Rotate 90 /Resources << /Font << /F 7 0 R >> >> /Contents 6 0 R >>",
        stream("",b"BT /F 12 Tf 10 70 Td (Web text) Tj ET /OC /Hidden BDC BT /F 12 Tf 10 50 Td (HIDDEN) Tj ET EMC"),
        stream("",b"BT /F 12 Tf 10 70 Td (Second page) Tj ET"),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Type /OCG /Name (Hidden text) >>",
        b"<< /Type /Outlines /First 10 0 R /Last 10 0 R /Count 1 >>",
        b"<< /Title (Chapter two) /Parent 9 0 R /Dest [4 0 R /Fit] >>",
        b"<< /Type /Annot /Subtype /Link /Rect [10 10 70 25] /Dest [4 0 R /Fit] >>",
        b"<< /Type /Annot /Subtype /Link /Rect [80 10 150 25] /A << /S /URI /URI (https://example.org/) >> >>",
        b"<< /Type /Annot /Subtype /Link /Rect [10 30 70 45] /A << /S /URI /URI (javascript:alert(1)) >> >>"]
    path=OUT/"web-features.pdf";path.write_bytes(pdf(objects));cases["web-features"]=path
    # Deliberately costly but bounded original content for real Worker hard-cancel tests.
    heavy=pdf([b"<< /Type /Catalog /Pages 2 0 R >>",b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 600] /Resources << /ExtGState << /G << /ca 0.5 >> >> >> /Contents 4 0 R >>",
        stream("",b"0.2 0.4 0.8 rg "+b"q /G gs 0 0 600 600 re f Q\n"*3000)])
    (OUT/"worker-heavy.pdf").write_bytes(heavy)
    subprocess.run(["cargo", "run", "--locked", "-p", "web-wasm", "--example", "interaction_fixtures", "--", str(OUT)], cwd=ROOT, check=True)
    subprocess.run(["cargo", "run", "--locked", "-p", "web-wasm", "--example", "content_fixtures", "--", str(OUT)], cwd=ROOT, check=True)
    subprocess.run(["cargo", "run", "--locked", "-p", "web-wasm", "--example", "font_write_fixtures", "--", str(OUT)], cwd=ROOT, check=True)
    subprocess.run(["cargo", "run", "--locked", "-p", "web-wasm", "--example", "language_fixtures", "--", str(OUT)], cwd=ROOT, check=True)
    Image.new("RGB", (2, 2), (0, 100, 220)).save(OUT / "content-replacement.jpg", "JPEG", quality=95)
    manifest = {name: {"path": str(path.relative_to(ROOT)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for name,path in cases.items()}
    (OUT/"manifest.json").write_text(json.dumps(manifest,indent=2)+"\n")
    print(json.dumps(manifest,indent=2))
