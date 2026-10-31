#!/usr/bin/env python3
"""Private, hash-pinned corpus runner; no downloads, Poppler, or corpus uploads.

Use the benchmark Python environment (PyMuPDF + Pillow). Each document has an
isolated native process and a deadline. Reports distinguish untested pages,
actual failures and visual review signals. Native limits are not Wasm evidence.
"""
import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
import subprocess
import threading
import time


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def probe(case, args):
    from PIL import Image, ImageChops, ImageStat
    import fitz
    folder = args.output / case['id']
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / 'report.json'
    report = dict(id=case['id'], sha256=case['sha256'], referencePages=case['referencePages'],
                  actualWasm=False, byteInputLengthFits=case['bytes'] <= 32 * 1024 * 1024,
                  blobInputLengthFits=case['bytes'] <= 2 * 1024 * 1024 * 1024,
                  operations=[], visual=[], status='failed')
    if digest(Path(case['path'])) != case['sha256']:
        report['reason'] = 'input_hash_changed'
    else:
        reference = fitz.open(case['path'])
        with (folder / 'stderr.log').open('w') as errors:
            start = time.monotonic()
            process = subprocess.Popen([str(args.probe), case['path'], str(folder)],
                                       stdout=subprocess.PIPE, stderr=errors, text=True)
            timed_out = threading.Event()
            def expire():
                timed_out.set()
                process.kill()
            timer = threading.Timer(args.timeout, expire)
            timer.start()
            try:
                with (folder / 'events.jsonl').open('w') as events:
                    for line in process.stdout:
                        events.write(line)
                        events.flush()
                        try:
                            event = json.loads(line)
                        except ValueError:
                            report.setdefault('invalidOutput', []).append(line[:256])
                            continue
                        report['operations'].append(event)
                        if event.get('operation') != 'render' or event['result']['status'] != 'passed':
                            continue
                        index = event['page']
                        image = folder / f'{index}.png'
                        try:
                            pix = reference[index].get_pixmap(matrix=fitz.Matrix(event['result']['dpi']/72,
                                                                                event['result']['dpi']/72), alpha=False)
                            expected = Image.frombytes('RGB', (pix.width, pix.height), pix.samples)
                            with Image.open(image) as source:
                                actual = source.convert('RGB')
                            size_delta = max(abs(a-b) for a,b in zip(actual.size,expected.size))
                            actual = actual.resize((256,256), Image.Resampling.BOX)
                            expected = expected.resize((256,256), Image.Resampling.BOX)
                            mae = sum(ImageStat.Stat(ImageChops.difference(actual, expected)).mean)/765
                            review = size_delta > 2 or mae > .08
                            report['visual'].append(dict(page=index, normalizedMae=mae,
                                                         dimensionDelta=size_delta, needsReview=review))
                            if review:
                                expected.save(folder / f'{index}-reference.png')
                            elif not args.keep_images:
                                image.unlink()
                        except Exception as error:
                            report['visual'].append(dict(page=index,error=str(error),needsReview=True))
                process.wait()
            finally:
                timer.cancel()
                if process.poll() is None:
                    process.kill()
                    process.wait()
                reference.close()
            report.update(exitCode=process.returncode, timedOut=timed_out.is_set(),
                          seconds=time.monotonic()-start)
            completed = any(e['operation']=='complete' for e in report['operations'])
            failures = [e for e in report['operations'] if e.get('result',{}).get('status')=='failed']
            report['status'] = 'passed' if completed and not failures and process.returncode==0 else 'failed'
    target.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest',type=Path)
    parser.add_argument('--probe',type=Path,default=Path('target/debug/examples/compatibility_probe'))
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--timeout',type=float,default=300,help='per-document wall seconds, including visual screening')
    parser.add_argument('--jobs',type=int,default=2)
    parser.add_argument('--keep-images',action='store_true')
    parser.add_argument('--require-clean',action='store_true',help='fail on any untested/failed page or visual review signal')
    args=parser.parse_args()
    if args.jobs<1 or args.timeout<=0:
        parser.error('jobs and timeout must be positive')
    args.probe=args.probe.resolve()
    args.output.mkdir(parents=True,exist_ok=True)
    manifest=json.loads(args.manifest.read_text())
    cases=manifest if isinstance(manifest,list) else manifest['cases']
    report=dict(actualWasm=False,reference='MuPDF overview screening; not visual proof',
                manifestSha256=digest(args.manifest),probeSha256=digest(args.probe),documents=[])
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures={pool.submit(probe,case,args):case for case in cases}
        for future in concurrent.futures.as_completed(futures):
            case=futures[future]
            try:
                result=future.result()
            except Exception as error:
                result=dict(id=case['id'],referencePages=case['referencePages'],status='failed',reason=str(error),operations=[],visual=[])
            report['documents'].append(result)
            print(json.dumps(dict(id=result['id'],status=result['status'],completed=len(report['documents']),total=len(cases))),flush=True)
            (args.output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    counts={'documents':len(cases),'pages':sum(c['referencePages'] for c in cases),
            'passedDocuments':sum(r['status']=='passed' for r in report['documents'])}
    for operation in ['render','text']:
        counts[operation+'PassedPages']=sum(e['operation']==operation and e['result']['status']=='passed'
            for r in report['documents'] for e in r['operations'] if 'result' in e)
        counts[operation+'FailedPages']=sum(e['operation']==operation and e['result']['status']=='failed'
            for r in report['documents'] for e in r['operations'] if 'result' in e)
        counts[operation+'UntestedPages']=counts['pages']-counts[operation+'PassedPages']-counts[operation+'FailedPages']
    counts['visualReviewPages']=sum(v['needsReview'] for r in report['documents'] for v in r['visual'])
    report['counts']=counts
    report['status']='passed' if counts['passedDocuments']==len(cases) and not counts['visualReviewPages'] and not counts['renderUntestedPages'] and not counts['textUntestedPages'] else 'failed'
    (args.output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(counts,indent=2))
    return int(args.require_clean and report['status']!='passed')


if __name__=='__main__':
    raise SystemExit(main())
