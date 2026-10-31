#!/usr/bin/env python3
"""Run isolated browser document shards and merge hash-matched evidence.

Uses existing Node, Playwright, Chromium and reference Python installations.
Private corpus inputs never leave this machine. Interrupted shards leave partial
JSONL evidence, but cannot produce a successful complete aggregate report.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

REPO = Path(__file__).resolve().parents[1]
OPERATIONS = ('open', 'render', 'text', 'saveReopen', 'editRoundtrip')


def aggregate(manifest, reports):
    expected = {case['id']: case for case in manifest['cases']}
    if not expected:
        raise ValueError('Empty corpus is not compatibility evidence')
    if len(expected) != len(manifest['cases']):
        raise ValueError('Duplicate corpus document identity')
    documents = {}
    identity = None
    for report in reports:
        current = {key: report[key] for key in ('manifestSha256', 'artifacts',
                   'driverSha256', 'referenceDriverSha256', 'browser', 'referenceEngines')}
        current['fontConfiguration'] = report.get('fontConfiguration')
        current['browserHarnessSha256'] = report.get('browserHarnessSha256')
        if identity is not None and identity != current:
            raise ValueError('Cannot merge evidence from different builds, drivers or engines')
        identity = current
        for document in report['documents']:
            ident = document['id']
            if ident not in expected or ident in documents:
                raise ValueError('Unexpected or duplicate document: ' + ident)
            if document['sha256'] != expected[ident]['sha256']:
                raise ValueError('Input hash mismatch: ' + ident)
            if document['referencePages'] != expected[ident]['referencePages']:
                raise ValueError('Page denominator mismatch: ' + ident)
            for event in document.get('operations', []):
                if event['operation'] in ('render', 'text'):
                    page = event.get('page')
                    if type(page) is not int or not 0 <= page < expected[ident]['referencePages']:
                        raise ValueError('Invalid page outcome: ' + ident)
            visual_pages = [v['page'] for v in document.get('visual', [])]
            if len(visual_pages) != len(set(visual_pages)) or any(type(page) is not int or not 0 <= page < expected[ident]['referencePages'] for page in visual_pages):
                raise ValueError('Invalid or duplicate visual outcome: ' + ident)
            documents[ident] = document
    counts = dict(documents=len(expected), pages=sum(c['referencePages'] for c in expected.values()))
    events = [event for doc in documents.values() for event in doc.get('operations', [])]
    for operation in OPERATIONS:
        observed = [(doc['id'], event.get('page')) for doc in documents.values()
                    for event in doc.get('operations', []) if event['operation'] == operation]
        if len(observed) != len(set(observed)):
            raise ValueError('Duplicate operation outcomes: ' + operation)
        denominator = counts['pages'] if operation in ('render', 'text') else counts['documents']
        for status in ('passed', 'failed'):
            counts[operation + status.title()] = sum(e['operation'] == operation and
                e['result']['status'] == status for e in events)
        counts[operation + 'Untested'] = denominator - counts[operation + 'Passed'] - counts[operation + 'Failed']
        if counts[operation + 'Untested'] < 0:
            raise ValueError('Operation count exceeds corpus denominator')
    visual = [item for doc in documents.values() for item in doc.get('visual', [])]
    counts['visualAttemptedPages'] = len(visual)
    counts['visualReferenceErrorPages'] = sum(bool(item.get('error')) for item in visual)
    counts['visualScreenedPages'] = sum(not item.get('error') and len(item.get('comparisons', [])) == 2 for item in visual)
    counts['visualUntestedPages'] = counts['pages'] - counts['visualScreenedPages']
    counts['textMissingUnicodeCharacters'] = sum(e['result'].get('missingUnicode', 0) for e in events if e['operation'] == 'text' and e['result']['status'] == 'passed')
    counts['textPagesWithMissingUnicode'] = sum(e['result'].get('missingUnicode', 0) > 0 for e in events if e['operation'] == 'text' and e['result']['status'] == 'passed')
    counts['visualReviewPages'] = sum(v.get('needsReview', True) for doc in documents.values() for v in doc.get('visual', []))
    counts['readableDocuments'] = sum(
        not doc.get('pageCountMismatch', True) and
        sum(e['operation'] == 'render' and e['result']['status'] == 'passed' for e in doc['operations']) == expected[ident]['referencePages']
        for ident, doc in documents.items())
    counts['renderCompleteDocuments'] = counts['readableDocuments']
    counts['screenedWithoutReviewDocuments'] = sum(
        not doc.get('pageCountMismatch', True) and len(doc.get('visual', [])) == expected[ident]['referencePages']
        and all(not v.get('error') and not v.get('needsReview', True) and len(v.get('comparisons', [])) == 2 for v in doc.get('visual', []))
        for ident, doc in documents.items())
    complete = len(documents) == len(expected) and not any(counts[op+'Untested'] for op in OPERATIONS) and all(doc.get('complete') is True for doc in documents.values())
    clean = complete and not any(counts[op+'Failed'] for op in OPERATIONS) and counts['visualReviewPages'] == 0 and counts['visualScreenedPages'] == counts['renderPassed']
    return dict(**(identity or {}), actualWasm=True, counts=counts,
                complete=complete, status='passed' if clean and all(d['status']=='passed' for d in documents.values()) else 'failed',
                missingDocuments=sorted(expected.keys()-documents.keys()),
                documents=[documents[c['id']] for c in manifest['cases'] if c['id'] in documents])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--jobs', type=int, default=4)
    parser.add_argument('--web', type=Path, default=REPO/'web')
    parser.add_argument('--node', default='node')
    parser.add_argument('--require-clean', action='store_true')
    parser.add_argument('--merge-only', action='store_true', help='recompute summaries from existing shard evidence; never infer missing outcomes')
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        parser.error('jobs must be 1..8')
    args.output.mkdir(parents=True, exist_ok=True)
    manifest_bytes = args.manifest.read_bytes()
    manifest = json.loads(manifest_bytes)
    if args.merge_only:
        existing = args.output/'report.json'
        exits = {item['index']: item['exitCode'] for item in json.loads(existing.read_text()).get('shards', [])} if existing.exists() else {}
        reports = [json.loads((args.output/str(i)/'report.json').read_text()) for i in range(args.jobs)]
        result = aggregate(manifest, reports)
        if result.get('manifestSha256') != hashlib.sha256(manifest_bytes).hexdigest():
            raise ValueError('Manifest identity changed')
        result['shards'] = [dict(index=i, exitCode=exits.get(i)) for i in range(args.jobs)]
        if any(exits.get(i) != 0 for i in range(args.jobs)):
            result.update(status='failed', complete=False)
        result['summaryDriverSha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        existing.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
        print(json.dumps(result['counts'], indent=2))
        return int(args.require_clean and result['status'] != 'passed')
    processes = []
    def shard(index):
        folder = args.output / str(index)
        folder.mkdir(exist_ok=True)
        env = dict(os.environ, RZPDF_COMPAT_MANIFEST=str(args.manifest.resolve()),
                   RZPDF_COMPAT_OUTPUT=str(folder.resolve()), RZPDF_COMPAT_WEB=str(args.web.resolve()),
                   RZPDF_COMPAT_SHARDS=str(args.jobs), RZPDF_COMPAT_SHARD_INDEX=str(index),
                   RZPDF_COMPAT_REQUIRE_CLEAN='0')
        with (folder/'driver.log').open('w') as log:
            process = subprocess.Popen([args.node, str(REPO/'web/tests/compatibility-corpus-browser.mjs')],
                                       cwd=REPO, env=env, stdout=log, stderr=subprocess.STDOUT)
            processes.append(process)
            code = process.wait()
        report = json.loads((folder/'report.json').read_text())
        report['driverExitCode'] = code
        return report
    start = time.monotonic()
    pool = concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs)
    futures = [pool.submit(shard, i) for i in range(args.jobs)]
    try:
        while not all(f.done() for f in futures):
            done, pending = concurrent.futures.wait(futures, timeout=10)
            print(json.dumps(dict(completedShards=len(done), totalShards=args.jobs,
                                  seconds=round(time.monotonic()-start))), flush=True)
        reports = [f.result() for f in futures]
        result = aggregate(manifest, reports)
        if result['manifestSha256'] != hashlib.sha256(manifest_bytes).hexdigest():
            raise ValueError('Manifest identity changed')
        result['shards'] = [dict(index=i, exitCode=r['driverExitCode']) for i,r in enumerate(reports)]
        if any(r['driverExitCode'] for r in reports):
            result.update(status='failed', complete=False)
        (args.output/'report.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
        print(json.dumps(result['counts'], indent=2))
        return int(args.require_clean and result['status'] != 'passed')
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
        pool.shutdown(wait=True, cancel_futures=True)


if __name__ == '__main__':
    raise SystemExit(main())
