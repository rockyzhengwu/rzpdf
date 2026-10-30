#!/usr/bin/env python3
"""Deterministic large RGB/Gray Flate + Gray SMask fixtures; stdlib only."""
import hashlib
import json
import sys
import zlib
from pathlib import Path


def stream(attrs, payload):
    return f'<< {attrs} /Length {len(payload)} >>\nstream\n'.encode() + payload + b'\nendstream'


def make_pdf(base, mask, background):
    # Same image is reused on white, dark and checkerboard backgrounds. A second
    # empty page verifies recovery after failed image decode.
    # Page extents and checker edges align at all three test DPIs, so vector
    # edge antialiasing differences do not obscure mask/color comparisons.
    pattern = b''
    if background == 'dark':
        pattern = b'0.1 0.1 0.1 rg 0 0 768 552 re f\n'
    elif background == 'checker':
        for y in range(0, 552, 32):
            for x in range(0, 768, 32):
                v = '.25' if (x // 32 + y // 32) % 2 else '.75'
                pattern += f'{v} g {x} {y} 32 32 re f\n'.encode()
    content = pattern + b'q 768 0 0 552 0 0 cm /Im1 Do Q'
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>',
               b'<< /Type /Pages /Count 2 /Kids [3 0 R 8 0 R] >>',
               b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 768 552] /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>',
               stream('', content), base, mask, stream('', b'0 0 1 rg 20 20 40 40 re f'),
               b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 768 552] /Resources << >> /Contents 7 0 R >>']
    pdf = bytearray(b'%PDF-1.7\n%\xe2\xe3\xcf\xd3\n')
    offsets = [0]
    for i, obj in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf.extend(f'{i} 0 obj\n'.encode() + obj + b'\nendobj\n')
    start = len(pdf)
    pdf.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:
        pdf.extend(f'{offset:010} 00000 n \n'.encode())
    pdf.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode())
    return bytes(pdf)


def generate(out):
    out.mkdir(parents=True, exist_ok=True)
    w, h = 2665, 1916
    # Solid interior probes plus sharp transparency transitions and a gradient.
    colors = bytearray()
    alpha = bytearray()
    for y in range(h):
        for x in range(w):
            a = 0 if x < w // 4 else (128 if x < w // 2 else (255 if x < w * 3 // 4 else round((x - w * 3 // 4) * 255 / (w - w * 3 // 4 - 1))))
            colors.extend((220, 40, 80) if a else (0, 255, 0))
            alpha.append(a)
    mask_prefix = f'/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode'
    color_prefix = f'/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /SMask 6 0 R'
    compressed = zlib.compress(colors)
    mask_raw = zlib.compress(alpha)
    images = {}
    for background in ('white', 'dark', 'checker'):
        images[f'soft-{background}.pdf'] = make_pdf(stream(color_prefix, compressed), stream(mask_prefix, mask_raw), background)
    # Matte white: a half-transparent red with a fully transparent white region.
    premixed = bytes((255, 127, 127)) * (w // 2) + bytes((255, 255, 255)) * (w - w // 2)
    matte_raw = zlib.compress(premixed * h)
    matte_alpha = zlib.compress((bytes([128]) * (w // 2) + bytes(w - w // 2)) * h)
    images['soft-matte.pdf'] = make_pdf(stream(color_prefix, matte_raw), stream(mask_prefix + ' /Matte [1 1 1]', matte_alpha), 'dark')
    # Independent dimensions and inverse Decode; alpha varies left to right.
    tiny = bytes([255, 127, 0, 255, 127, 0])
    tiny_prefix = '/Type /XObject /Subtype /Image /Width 3 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Decode [1 0]'
    images['soft-different.pdf'] = make_pdf(stream(color_prefix, compressed), stream(tiny_prefix, zlib.compress(tiny)), 'dark')
    bad = bytearray(mask_raw)
    bad[-1] ^= 1
    images['soft-corrupt.pdf'] = make_pdf(stream(color_prefix, compressed), stream(mask_prefix, bytes(bad)), 'white')
    manifest = {}
    for name, pdf in images.items():
        (out / name).write_bytes(pdf)
        manifest[name] = dict(bytes=len(pdf), sha256=hashlib.sha256(pdf).hexdigest())
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    generate(Path(sys.argv[1]))
