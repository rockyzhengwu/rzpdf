#!/usr/bin/env python3
"""Freeze a local real-PDF inventory. Never copy PDFs or commit private paths.

Requires PyMuPDF in the benchmark Python environment. Producer classifications
are heuristics; they are not a representative-population sampling guarantee.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import fitz


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('roots',nargs='+',type=Path)
    parser.add_argument('--output',required=True,type=Path)
    args=parser.parse_args()
    cases=[]
    seen=set()
    for root in args.roots:
        paths=[root] if root.is_file() else sorted(root.rglob('*'))
        for path in paths:
            if not path.is_file() or path.suffix.lower()!='.pdf':
                continue
            sha=hashlib.sha256()
            with path.open('rb') as stream:
                for block in iter(lambda:stream.read(1024*1024),b''):
                    sha.update(block)
            sha=sha.hexdigest()
            if sha in seen:
                continue
            seen.add(sha)
            entry=dict(id=sha[:16],path=str(path.resolve()),sha256=sha,bytes=path.stat().st_size)
            try:
                with fitz.open(path) as pdf:
                    metadata=pdf.metadata or {}
                    producer=metadata.get('producer','')
                    creator=metadata.get('creator','')
                    hint=(producer+' '+creator).lower()
                    category=next((name for name,terms in [
                        ('tex',['tex']),('office',['word','office','excel','powerpoint','libreoffice']),
                        ('browser',['chrom','webkit','skia']),('adobe',['adobe','acrobat','distiller'])
                    ] if any(term in hint for term in terms)),'other')
                    entry.update(referencePages=len(pdf),passwordRequired=bool(pdf.needs_pass),
                                 producer=producer,creator=creator,category=category)
            except Exception as error:
                entry.update(referencePages=0,inventoryError=str(error),category='unreadable')
            cases.append(entry)
    report=dict(schema=1,kind='private-local-real-documents',created=datetime.date.today().isoformat(),cases=cases)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(dict(documents=len(cases),pages=sum(c['referencePages'] for c in cases),
                         bytes=sum(c['bytes'] for c in cases)),indent=2))


if __name__=='__main__':
    main()
