#!/usr/bin/env python3
"""Read-only PDFium corpus adapter; reports are diagnostic, not conformance scores."""
import argparse
import collections
import concurrent.futures
import hashlib
import html
import json
import os
from pathlib import Path
import random
import shutil
import subprocess
import sys

import sample_pdf_corpus as common


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def reference_worker(source, output, page, dpi):
    import pypdfium2 as pdfium
    with pdfium.PdfDocument(source) as doc:
        print(f'pages={len(doc)}', flush=True)
        sheet = doc[int(page)-1]
        try:
            bitmap = sheet.render(scale=float(dpi)/72, draw_annots=False)
            try:
                bitmap.to_pil().save(output)
            finally:
                bitmap.close()
        finally:
            sheet.close()


def check(item, args):
    path = Path(item['path'])
    parsed = common.parsing.check(path, args)
    folder = args.output / item['id']
    folder.mkdir()
    first = folder / '1-pdfium.png'
    baseline = common.run(args.command_prefix + [sys.executable, __file__, '--reference-worker', str(path), str(first), '1', str(args.dpi)], args.render_timeout)
    import re
    match = re.search(r'^pages=(\d+)', baseline['stdout'], re.M)
    pages = int(match[1]) if match else parsed.get('pages', 0)
    renders = []
    for number in (range(1,pages+1) if args.pages=='all' else sorted({1, (pages+1)//2, pages})) if pages else []:
        actual = folder / f'{number}-rzpdf.png'
        reference = folder / f'{number}-pdfium.png'
        ref = baseline if number == 1 else common.run(args.command_prefix + [sys.executable, __file__, '--reference-worker', str(path), str(reference), str(number), str(args.dpi)], args.render_timeout)
        result = common.run(args.command_prefix + [str(args.render_binary), str(path), str(actual), str(number), args.mode, str(args.dpi)], args.render_timeout)
        row = dict(page=number, rzpdf=result, pdfium=ref, actual=str(actual), reference=str(reference))
        if result['status'] == ref['status'] == 'ok':
            try:
                row['comparison'] = common.compare(actual, reference)
                from PIL import ImageChops, ImageEnhance
                a, b = common.rgb_on_white(actual), common.rgb_on_white(reference)
                heat = ImageChops.difference(a.resize(b.size), b)
                ImageEnhance.Contrast(heat).enhance(4).save(folder / f'{number}-diff.png')
            except (OSError, ValueError) as error:
                row['comparison_error'] = str(error)
        renders.append(row)
    return dict(**item, parsing=parsed, reference_probe=baseline, reference_pages=pages, renders=renders)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pdfium', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--count', type=int, default=120, help='0 selects all')
    parser.add_argument('--manifest', type=Path, help='Replay a prior manifest; verifies input hashes')
    parser.add_argument('--pages', choices=['sample','all'], default='sample')
    parser.add_argument('--seed', type=int, default=20261022)
    parser.add_argument('--memory-mib', type=int, default=2048, help='Linux per-worker virtual address-space cap')
    parser.add_argument('--jobs', type=int, default=3)
    parser.add_argument('--timeout', type=float, default=20)
    parser.add_argument('--render-timeout', type=float, default=20)
    parser.add_argument('--dpi', type=float, default=48)
    parser.add_argument('--mode', choices=['strict','compatible'], default='compatible')
    args = parser.parse_args()
    if args.count < 0 or min(args.jobs,args.timeout,args.render_timeout,args.dpi,args.memory_mib)<=0:
        parser.error('invalid limits')
    if not shutil.which('prlimit'):
        parser.error('prlimit is required for isolated worker memory limits')
    args.command_prefix=['prlimit',f'--as={args.memory_mib*1024*1024}','--']
    args.output = args.output.resolve()
    args.pdfium = args.pdfium.resolve()
    resources = args.pdfium / 'testing/resources'
    if not resources.is_dir():
        parser.error('PDFium testing/resources missing')
    args.output.mkdir(parents=True, exist_ok=False)
    generated = args.output / 'inputs'
    generated.mkdir()
    candidates, preparation_errors = [], []
    # Prefer templates when a checked-in PDF has the same stem; no duplicates.
    sources = sorted(p for p in resources.rglob('*') if p.suffix in ('.pdf','.in') and not (p.suffix=='.pdf' and p.with_suffix('.in').exists()))
    if args.manifest:
        candidates = [json.loads(line) for line in args.manifest.read_text().splitlines() if line.strip()]
        for item in candidates:
            if sha(item["path"])!=item["sha256"]:raise ValueError("manifest input changed")
        sources=[]
    for index, source in enumerate(sources):
        relative = str(source.relative_to(resources))
        item = dict(id=f'{index:04d}', source=relative, source_sha256=sha(source))
        path = source
        if source.suffix == '.in':
            dest = generated / item['id']; dest.mkdir()
            expanded = common.run([sys.executable,str(args.pdfium/'testing/tools/fixup_pdf_template.py'),'--output-dir',str(dest),str(source)],args.timeout)
            path = dest / (source.stem+'.pdf')
            if expanded['status']!='ok' or not path.is_file():
                preparation_errors.append(dict(**item,result=expanded));continue
        candidates.append(dict(**item,path=str(path),sha256=sha(path)))
    # Round-robin independently shuffled categories avoids an alphabetic sample.
    groups = collections.defaultdict(list)
    for item in candidates:
        category = item['source'].split('/')[0] if '/' in item['source'] else 'root'
        groups[category].append(item)
    rng = random.Random(args.seed)
    for group in groups.values(): rng.shuffle(group)
    chosen=[]
    while any(groups.values()) and (not args.count or len(chosen)<args.count):
        for key in sorted(groups):
            if groups[key] and (not args.count or len(chosen)<args.count): chosen.append(groups[key].pop())
    workers = args.output/'workers'; workers.mkdir()
    fingerprints={}
    for attr,name in [('binary','check_pdf'),('render_binary','check_render')]:
        target=workers/name;shutil.copy2(common.REPO/'target/release/examples'/name,target)
        setattr(args,attr,target);fingerprints[name]=sha(target)
    import pypdfium2 as pdfium
    import pypdfium2_raw as raw
    libraries={str(p):sha(p) for p in Path(raw.__file__).parent.glob('*pdfium*.so')}
    settings=dict(pdfium_checkout_commit=subprocess.check_output(['git','-C',str(args.pdfium),'rev-parse','HEAD'],text=True).strip(),
        reference_pdfium=str(pdfium.PDFIUM_INFO),reference_binding=str(pdfium.PYPDFIUM_INFO),reference_libraries=libraries,
        runner_sha256=sha(__file__),comparison_tool_sha256=sha(common.__file__),
        substitute_font_sha256=sha(os.environ['RZPDF_SUBSTITUTE_FONT']) if os.environ.get('RZPDF_SUBSTITUTE_FONT') else None,
        pdfium_checkout_dirty=subprocess.check_output(['git','-C',str(args.pdfium),'status','--porcelain'],text=True),
        template_tool_sha256=sha(args.pdfium/'testing/tools/fixup_pdf_template.py'),workers=fingerprints,
        worker_address_space_mib=args.memory_mib,candidates=len(candidates),selected=len(chosen),seed=args.seed,mode=args.mode,dpi=args.dpi,
        substitute_font=os.environ.get('RZPDF_SUBSTITUTE_FONT'),render_pages=args.pages,source_manifest=str(args.manifest) if args.manifest else None,
        reference_annotations=False,optional_content='default View configuration and usage',scope='diagnostic; malformed and unsupported feature tests require triage')
    (args.output/'settings.json').write_text(json.dumps(settings,indent=2)+'\n')
    (args.output/'manifest.jsonl').write_text(''.join(json.dumps(i)+'\n' for i in chosen))
    (args.output/'preparation-errors.json').write_text(json.dumps(preparation_errors,indent=2)+'\n')
    rows=[]
    with (args.output/'results.jsonl').open('x') as report:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for row in pool.map(lambda item:check(item,args),chosen):
                rows.append(row);report.write(json.dumps(row)+'\n');report.flush()
                print(f"[{len(rows)}/{len(chosen)}] {row['source']}: {row['parsing']['status']}",flush=True)
    renders=[r for row in rows for r in row['renders']]
    summary=dict(files=len(rows),preparation_errors=len(preparation_errors),
        page_count_mismatches=sum(r['parsing'].get('pages') is not None and r['parsing']['pages']!=r['reference_pages'] for r in rows),
        parsing=dict(collections.Counter(r['parsing']['status'] for r in rows)),
        renders=dict(collections.Counter(r['rzpdf']['status'] for r in renders)),
        reference=dict(collections.Counter(r['pdfium']['status'] for r in renders)),
        compared=sum('comparison' in r for r in renders),
        visual_review=sum(r.get('comparison',{}).get('needs_visual_review',False) for r in renders))
    (args.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    blocks=['<!doctype html><meta charset="utf-8"><title>PDFium corpus diagnostics</title><style>body{font:16px sans-serif}img{max-width:30%;vertical-align:top}pre{white-space:pre-wrap}</style>', '<h1>PDFium corpus diagnostics</h1><pre>'+html.escape(json.dumps(summary,indent=2))+'</pre><p>Differences require review; expected-invalid and unsupported-feature inputs are not automatically defects.</p>']
    for row in rows:
        suspicious=row['parsing']['status']!='ok' or any(r['rzpdf']['status']!='ok' or r.get('comparison',{}).get('needs_visual_review') for r in row['renders'])
        if not suspicious:continue
        blocks.append('<h2>'+html.escape(row['source'])+'</h2><pre>'+html.escape(row['parsing']['details'][:2000])+'</pre>')
        for r in row['renders']:
            blocks.append('<p>Page '+str(r['page'])+'; rzpdf / PDFium / difference</p><pre>'+html.escape(json.dumps(r.get('comparison',{})))+'\n'+html.escape(r['rzpdf']['stderr'][:1500])+'</pre>')
            for suffix in ('rzpdf','pdfium','diff'):
                image=args.output/row['id']/f"{r['page']}-{suffix}.png"
                if image.exists():blocks.append('<img loading="lazy" src="'+str(image.relative_to(args.output))+'">')
    (args.output/'report.html').write_text('\n'.join(blocks))
    print(json.dumps(summary),flush=True)


if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--reference-worker':reference_worker(*sys.argv[2:])
    else:main()
