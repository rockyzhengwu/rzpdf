#!/usr/bin/env python3
"""Compare corpus runs without turning diagnostic errors into expected failures."""
import argparse
import collections
import json
from pathlib import Path


def phase(message):
    if any(word in message for word in ('JPX','JBIG2','image','Mask','Color','Function','shading','ICC','pattern','CCITT')):
        return 'PF-02'
    if any(word in message for word in ('Font','font','CMap','ToUnicode','CID','Type3')):
        return 'PF-03'
    return 'PF-04'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before',required=True,type=Path)
    parser.add_argument('--after',required=True,type=Path)
    parser.add_argument('--output',required=True,type=Path)
    args=parser.parse_args()
    before={r['id']:r for r in map(json.loads,args.before.read_text().splitlines())}
    after={r['id']:r for r in map(json.loads,args.after.read_text().splitlines())}
    if before.keys()!=after.keys():raise ValueError('corpus sets differ')
    entries=[]
    for key,row in after.items():
        old=before[key]
        if row['sha256']!=old['sha256']:raise ValueError('input identity changed')
        failures=[]
        for page in row['renders']:
            if page['rzpdf']['status']!='ok':failures.append({'stage':'render','page':page['page'],'message':page['rzpdf']['stderr'],'reference_status':page['pdfium']['status']})
        if row['parsing']['status'] in ('error','crash','timeout'):
            failures.append({'stage':'interpret','message':row['parsing']['details']})
        status='needs_triage' if failures else 'observed_pass'
        if not failures and row['parsing']['status']=='warning':status='observed_pass_with_diagnostics'
        visual=[p['page'] for p in row['renders'] if p.get('comparison',{}).get('needs_visual_review')]
        old_errors=old['parsing']['status']=='error' or any(p['rzpdf']['status']!='ok' for p in old['renders'])
        if old_errors and not failures:status='recovered_in_retest'
        messages='\n'.join(p['message'] for p in failures)
        # Explicit limitations remain open work, never a green expected-failure list.
        category='declared_unsupported' if 'UnsupportedFeature' in messages or 'UnsupportedSecurity' in messages else 'password_required' if 'PasswordRequired' in messages else 'resource_rejection' if 'ResourceLimit' in messages else 'unadjudicated'
        entries.append(dict(id=key,source=row['source'],sha256=row['sha256'],status=status,
            phase=phase(messages) if failures else 'PF-01' if visual else 'PF-05',
            category=category if failures else None,visual_review_pages=visual,
            failures=failures,warning_details=row['parsing']['details'] if row['parsing']['status']=='warning' else '',
            reference_pages=row['reference_pages'],interpreted_pages=row['parsing'].get('pages')))
    args.output.mkdir(parents=True,exist_ok=False)
    (args.output/'cases.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in entries))
    groups=collections.defaultdict(list)
    for row in entries:
        for error in row['failures']:
            lines=error['message'].strip().splitlines();message=lines[0] if lines else 'no diagnostic'
            groups[(row['phase'],message)].append({'id':row['id'],'stage':error['stage'],'page':error.get('page')})
    summary=dict(status_counts=dict(collections.Counter(r['status'] for r in entries)),
        open_category_counts=dict(collections.Counter(r['category'] for r in entries if r['failures'])),
        visual_review_cases=sum(bool(r['visual_review_pages']) for r in entries),
        complete=False,note='Observed passes are not conformance results. Unsupported and unadjudicated cases stay open.')
    (args.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    (args.output/'groups.json').write_text(json.dumps([dict(phase=p,message=m,cases=cases) for (p,m),cases in sorted(groups.items(),key=lambda item:-len(item[1]))],indent=2)+'\n')
    print(json.dumps(summary))


if __name__=='__main__':main()
