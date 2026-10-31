#!/usr/bin/env python3
"""Local JSONL visual screening helper for the actual-Wasm corpus runner.

One bounded page at a time; compare independently with MuPDF and PDFium.
Thresholds identify review candidates, not correctness proofs.
"""
import base64
import io
import json
import math
from pathlib import Path
import sys
import fitz
import pypdfium2 as pdfium
from PIL import Image, ImageChops, ImageStat


def main():
    manifest=json.loads(Path(sys.argv[1]).read_text())
    cases={case['id']:case for case in manifest['cases']}
    output=Path(sys.argv[2])
    print(json.dumps(dict(ready=True,mupdf=fitz.VersionBind,pdfium=str(pdfium.PDFIUM_INFO),
                          pypdfium=str(pdfium.PYPDFIUM_INFO),maxReviewPagesPerDocument=20,
                          maxStoredPngBytes=512*1024*1024)),flush=True)
    saved_pages={}
    stored_bytes=0
    def save(image,path):
        nonlocal stored_bytes
        buffer=io.BytesIO()
        image.save(buffer,format='PNG')
        data=buffer.getvalue()
        if stored_bytes+len(data)>512*1024*1024:
            return False
        path.write_bytes(data)
        stored_bytes+=len(data)
        return True
    current=None
    mupdf=other=None
    for line in sys.stdin:
        request=json.loads(line)
        try:
            case=cases[request['id']]
            if current!=case['id']:
                if mupdf is not None:
                    mupdf.close()
                    other.close()
                mupdf=fitz.open(case['path'])
                other=pdfium.PdfDocument(case['path'])
                current=case['id']
            width,height=request['width'],request['height']
            if not (0<width<=1024 and 0<height<=1024):
                raise ValueError('visual screening image exceeds bounds')
            raw=base64.b64decode(request['rgba'],validate=True)
            if len(raw)!=width*height*4:
                raise ValueError('invalid RGBA image length')
            actual=Image.frombytes('RGBA',(width,height),raw).convert('RGB')
            scale=request['dpi']/72
            source_page=mupdf[request['page']]
            if max(math.ceil(source_page.rect.width*scale),math.ceil(source_page.rect.height*scale))>1024:
                raise ValueError('MuPDF geometry exceeds bounded screening raster')
            pix=source_page.get_pixmap(matrix=fitz.Matrix(scale,scale),alpha=False,annots=False)
            reference=Image.frombytes('RGB',(pix.width,pix.height),pix.samples)
            page=other[request['page']]
            try:
                if max(math.ceil(value*scale) for value in page.get_size())>1024:
                    raise ValueError('PDFium geometry exceeds bounded screening raster')
                bitmap=page.render(scale=scale,draw_annots=False)
                try:
                    second=bitmap.to_pil().convert('RGB')
                finally:
                    bitmap.close()
            finally:
                page.close()
            comparisons=[]
            for name,expected in [('mupdf',reference),('pdfium',second)]:
                a=actual.resize((256,256),Image.Resampling.BOX)
                b=expected.resize((256,256),Image.Resampling.BOX)
                mae=sum(ImageStat.Stat(ImageChops.difference(a,b)).mean)/765
                dimension=max(abs(a-b) for a,b in zip(actual.size,expected.size))
                ink=lambda image:sum(image.convert('L').histogram()[:240])/(256*256)
                ai,bi=ink(a),ink(b)
                blank=(ai<.001)!=(bi<.001)
                ratio=ai/bi if bi else None
                review=dimension>2 or mae>.08 or blank or (ratio is not None and bi>.001 and not .5<=ratio<=2)
                comparisons.append(dict(reference=name,normalizedMae=mae,dimensionDelta=dimension,
                                        actualInk=ai,referenceInk=bi,needsReview=review))
                if review:
                    retained=saved_pages.setdefault(case['id'],set())
                    if request['page'] in retained or len(retained)<20:
                        folder=output/case['id']
                        folder.mkdir(parents=True,exist_ok=True)
                        actual_saved=True
                        if request['page'] not in retained:
                            actual_saved=save(actual,folder/f"{request['page']}-wasm.png")
                        retained.add(request['page'])
                        expected_saved=save(expected,folder/f"{request['page']}-{name}.png")
                        comparisons[-1]['artifactsOmitted']=not (actual_saved and expected_saved)
                    else:
                        comparisons[-1]['artifactsOmitted']=True
            print(json.dumps(dict(comparisons=comparisons,needsReview=any(c['needsReview'] for c in comparisons))),flush=True)
        except Exception as error:
            print(json.dumps(dict(error=str(error),needsReview=True)),flush=True)
    if mupdf is not None:
        mupdf.close()
        other.close()


if __name__=='__main__':
    main()
