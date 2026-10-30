#!/usr/bin/env python3
"""Render the fixed masked fixtures with independent MuPDF and PDFium engines."""
import hashlib
import ctypes
import math
import json
import sys
from pathlib import Path
import fitz
import pypdfium2 as pdfium

source, out = map(Path, sys.argv[1:])
out.mkdir(parents=True, exist_ok=True)
report = {'mupdf': fitz.VersionFitz, 'pdfium': str(pdfium.PDFIUM_INFO), 'transform': 'exact DPI matrix; ceil-sized white destination, no fit-to-bitmap stretch', 'files': []}
for path in sorted(source.glob('soft-*.pdf')):
    if 'corrupt' in path.name:
        continue
    for dpi in (72, 36, 18):
        for engine in ('mupdf', 'pdfium'):
            output = out / f'{path.stem}-{engine}-{dpi}.png'
            if engine == 'mupdf':
                with fitz.open(path) as doc:
                    image = doc[0].get_pixmap(matrix=fitz.Matrix(dpi / 72, dpi / 72), alpha=False)
                    image.save(output)
                    width, height = image.width, image.height
            else:
                doc = pdfium.PdfDocument(path)
                page = doc[0]
                scale = dpi / 72
                width, height = math.ceil(page.get_width() * scale), math.ceil(page.get_height() * scale)
                bitmap = pdfium.PdfBitmap.new_native(width, height, pdfium.raw.FPDFBitmap_BGRA, rev_byteorder=True)
                pdfium.raw.FPDFBitmap_FillRect(bitmap, 0, 0, width, height, 0xffffffff)
                # The integer-size RenderPageBitmap API stretches fractional
                # page extents to fill the rounded bitmap. Use the same exact
                # DPI matrix as MuPDF/rzpdf to avoid false checker-edge errors.
                matrix = pdfium.raw.FS_MATRIX(scale, 0, 0, scale, 0, 0)
                clip = pdfium.raw.FS_RECTF(0, 0, width, height)
                pdfium.raw.FPDF_RenderPageBitmapWithMatrix(bitmap, page, ctypes.byref(matrix), ctypes.byref(clip), pdfium.raw.FPDF_REVERSE_BYTE_ORDER)
                image = bitmap.to_pil()
                image.save(output)
                width, height = image.size
                bitmap.close()
                page.close()
                doc.close()
            report['files'].append(dict(name=output.name, width=width, height=height, sha256=hashlib.sha256(output.read_bytes()).hexdigest()))
(out / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(dict(status='passed', images=len(report['files']), mupdf=report['mupdf'],pdfium=report['pdfium'])))
