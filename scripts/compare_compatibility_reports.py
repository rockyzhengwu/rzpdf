#!/usr/bin/env python3
"""Compare measured operation outcomes; missing results are untested, not passes."""
import argparse
import collections
import json
from pathlib import Path


def outcomes(report):
    result={}
    for document in report['documents']:
        for event in document.get('operations',[]):
            if 'result' in event:
                result[(document['id'],event['operation'],event.get('page'))]=event['result']
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before',type=Path)
    parser.add_argument('after',type=Path)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--no-regressions',action='store_true')
    args=parser.parse_args()
    before=json.loads(args.before.read_text())
    after=json.loads(args.after.read_text())
    if before['manifestSha256']!=after['manifestSha256']:
        parser.error('corpus manifests differ; compare only the same frozen inputs')
    old,new=outcomes(before),outcomes(after)
    counts=collections.Counter()
    changes=[]
    groups=collections.Counter()
    regressions=0
    for key in sorted(old.keys()|new.keys(),key=str):
        a=old.get(key,{}).get('status','untested')
        b=new.get(key,{}).get('status','untested')
        counts[f'{a}->{b}']+=1
        if a!=b:
            changes.append(dict(document=key[0],operation=key[1],page=key[2],before=a,after=b))
        regressions+=a=='passed' and b!='passed'
        if b=='failed':
            value=new[key]
            resource=value.get('resource') or {}
            groups[(key[1],value.get('code','native'),resource.get('stage','unknown'),
                    resource.get('reason',value.get('message','unclassified')))]+=1
    report=dict(manifestSha256=after['manifestSha256'],transitions=dict(counts),
                regressions=regressions,changes=changes,
                currentFailureGroups=[dict(operation=k[0],code=k[1],stage=k[2],reason=k[3],count=n)
                                      for k,n in groups.most_common()],
                sameRuntime=before.get('actualWasm')==after.get('actualWasm'),
                performanceComparable=False)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(dict(transitions=report['transitions'],regressions=regressions),indent=2))
    return int(args.no_regressions and regressions>0)


if __name__=='__main__':
    raise SystemExit(main())
